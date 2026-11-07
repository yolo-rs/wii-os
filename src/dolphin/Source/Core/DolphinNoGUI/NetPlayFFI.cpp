// Copyright 2026 Wii OS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

#include "DolphinNoGUI/NetPlayFFI.h"

#include <algorithm>
#include <array>
#include <cstring>
#include <deque>
#include <mutex>
#include <string>
#include <utility>

#include "Common/TraversalClient.h"
#include "Core/Boot/Boot.h"
#include "Core/BootManager.h"
#include "Core/Core.h"
#include "Core/NetPlayClient.h"
#include "Core/NetPlayServer.h"
#include "Core/System.h"
#include "UICommon/GameFile.h"

class FfiNetPlaySession final : public NetPlay::NetPlayUI,
                                public std::enable_shared_from_this<FfiNetPlaySession>
{
public:
  explicit FfiNetPlaySession(WindowSystemInfo window_system_info)
      : m_wsi(std::move(window_system_info))
  {
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_READY, 0, "NetPlay host ready");
  }

  ~FfiNetPlaySession() override
  {
    // NetPlay destructors join their networking threads and call back into the UI.
    // Keep this adapter alive until both objects have finished teardown.
    m_client.reset();
    m_server.reset();
  }

  void Host(std::string game_path, const u16 port, std::string nickname)
  {
    if (m_client || m_server)
    {
      Fail("A NetPlay session is already active");
      return;
    }
    if (!LoadGame(game_path))
      return;

    NetPlay::NetTraversalConfig traversal{};
    m_server = std::make_unique<NetPlay::NetPlayServer>(port, false, this, traversal);
    if (!m_server->is_connected)
    {
      m_server.reset();
      Fail("Could not open the NetPlay host port");
      return;
    }
    m_is_hosting.store(true, std::memory_order_release);
    const auto game = m_game_file->GetSyncIdentifier();
    if (!m_server->ChangeGame(game, m_game_file->GetFileName()))
    {
      m_server.reset();
      m_is_hosting.store(false, std::memory_order_release);
      Fail("Could not select the host game");
      return;
    }

    // Player 1 is the local host. Dolphin initializes all mappings to zero.
    NetPlay::PadMappingArray pads{};
    pads[0] = 1;
    m_server->SetPadMapping(pads);
    NetPlay::PadMappingArray wiimotes{};
    wiimotes[0] = 1;
    m_server->SetWiimoteMapping(wiimotes);

    NetPlay::NetTraversalConfig direct{};
    m_client = std::make_unique<NetPlay::NetPlayClient>("127.0.0.1", port, this,
                                                        std::move(nickname), direct);
    if (!m_client->IsConnected())
    {
      m_client.reset();
      m_server.reset();
      m_is_hosting.store(false, std::memory_order_release);
      Fail("Could not connect the local host player");
      return;
    }
    m_client->ChangeGame(game_path);
    m_status.store(WII_DOLPHIN_NETPLAY_LOBBY, std::memory_order_release);
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_HOSTED, 0, "Hosting direct LAN NetPlay");
  }

  void Join(std::string game_path, std::string address, const u16 port, std::string nickname)
  {
    if (m_client || m_server)
    {
      Fail("A NetPlay session is already active");
      return;
    }
    if (!LoadGame(game_path))
      return;

    NetPlay::NetTraversalConfig direct{};
    m_client = std::make_unique<NetPlay::NetPlayClient>(address, port, this,
                                                        std::move(nickname), direct);
    if (!m_client->IsConnected())
    {
      m_client.reset();
      Fail("Could not connect to the NetPlay host");
      return;
    }
    m_client->ChangeGame(game_path);
    m_status.store(WII_DOLPHIN_NETPLAY_LOBBY, std::memory_order_release);
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_JOINED, 0, "Joined direct LAN NetPlay");
  }

  void StartGame()
  {
    if (!m_server || !m_client || !m_client->DoAllPlayersHaveGame())
    {
      Fail("Every player must have the same game before starting");
      return;
    }
    if (!m_server->RequestStartGame())
      Fail("Dolphin rejected the NetPlay start request");
  }

  int Status() const { return m_status.load(std::memory_order_acquire); }

  bool PollEvent(WiiDolphinFfiNetPlayEvent* event)
  {
    if (!event)
      return false;
    std::lock_guard lock(m_event_mutex);
    if (m_events.empty())
      return false;
    *event = m_events.front();
    m_events.pop_front();
    return true;
  }

  void Stop()
  {
    m_status.store(WII_DOLPHIN_NETPLAY_STOPPING, std::memory_order_release);
    m_client.reset();
    m_server.reset();
    m_is_hosting.store(false, std::memory_order_release);
    Core::System& system = Core::System::GetInstance();
    if (Core::IsRunning(system))
      Core::Stop(system);
    m_status.store(WII_DOLPHIN_NETPLAY_STOPPED, std::memory_order_release);
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_GAME_STOPPED, 0, "NetPlay stopped");
  }

  // NetPlayUI. Network-thread callbacks only touch atomics/event queue or enqueue
  // host-thread work; none invokes Core, BootManager, or Platform directly.
  void BootGame(const std::string& filename,
                std::unique_ptr<BootSessionData> boot_session_data) override
  {
    std::weak_ptr<FfiNetPlaySession> weak = weak_from_this();
    auto boot = std::shared_ptr<BootSessionData>(std::move(boot_session_data));
    Core::QueueHostJob([weak, filename, boot](Core::System&) mutable {
      const auto self = weak.lock();
      if (!self)
        return;
      auto parameters = BootParameters::GenerateFromFile(
          filename, boot ? std::move(*boot) : BootSessionData{});
      if (!parameters || !BootManager::BootCore(Core::System::GetInstance(),
                                                 std::move(parameters), self->m_wsi))
      {
        self->Fail("Dolphin could not boot the NetPlay game");
        return;
      }
      self->m_status.store(WII_DOLPHIN_NETPLAY_RUNNING, std::memory_order_release);
      self->PushEvent(WII_DOLPHIN_NETPLAY_EVENT_GAME_STARTED, 0, "NetPlay game started");
    }, true);
  }
  void StopGame() override
  {
    std::weak_ptr<FfiNetPlaySession> weak = weak_from_this();
    Core::QueueHostJob([weak](Core::System& system) {
      if (const auto self = weak.lock())
      {
        if (Core::IsRunning(system))
          Core::Stop(system);
        self->m_status.store(WII_DOLPHIN_NETPLAY_LOBBY, std::memory_order_release);
        self->PushEvent(WII_DOLPHIN_NETPLAY_EVENT_GAME_STOPPED, 0, "NetPlay game stopped");
      }
    }, true);
  }
  bool IsHosting() const override { return m_is_hosting.load(std::memory_order_acquire); }
  void Update() override {}
  void AppendChat(const std::string& message) override
  {
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_CHAT, 0, message);
  }
  void OnMsgChangeGame(const NetPlay::SyncIdentifier&, const std::string&) override {}
  void OnMsgChangeGBARom(int, const NetPlay::GBAConfig&) override {}
  void OnMsgStartGame() override
  {
    std::weak_ptr<FfiNetPlaySession> weak = weak_from_this();
    Core::QueueHostJob([weak](Core::System&) {
      if (const auto self = weak.lock(); self && self->m_client)
        self->m_client->StartGame(self->m_game_path);
    }, true);
  }
  void OnMsgStopGame() override {}
  void OnMsgPowerButton() override {}
  void OnPlayerConnect(const std::string& player) override
  {
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_PLAYER_JOINED, 0, player);
  }
  void OnPlayerDisconnect(const std::string& player) override
  {
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_PLAYER_LEFT, 0, player);
  }
  void OnPadBufferChanged(u32) override {}
  void OnHostInputAuthorityChanged(bool) override {}
  void OnDesync(u32 frame, const std::string& player) override
  {
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_ERROR, static_cast<int32_t>(frame),
              "Desync reported for " + player);
  }
  void OnConnectionLost() override
  {
    m_status.store(WII_DOLPHIN_NETPLAY_FAILED, std::memory_order_release);
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_CONNECTION_LOST, 0, "NetPlay connection lost");
  }
  void OnConnectionError(const std::string& message) override { Fail(message); }
  void OnTraversalError(Common::TraversalClient::FailureReason) override {}
  void OnTraversalStateChanged(Common::TraversalClient::State) override {}
  void OnGameStartAborted() override { Fail("NetPlay game start was aborted"); }
  void OnGolferChanged(bool, const std::string&) override {}
  void OnTtlDetermined(u8) override {}
  bool IsRecording() override { return false; }
  std::shared_ptr<const UICommon::GameFile>
  FindGameFile(const NetPlay::SyncIdentifier& sync,
               NetPlay::SyncIdentifierComparison* found = nullptr) override
  {
    if (!m_game_file)
      return {};
    const auto comparison = m_game_file->CompareSyncIdentifier(sync);
    if (found)
      *found = comparison;
    return comparison == NetPlay::SyncIdentifierComparison::SameGame ? m_game_file : nullptr;
  }
  std::string FindGBARomPath(const std::array<u8, 20>&, std::string_view, int) override { return {}; }
  void ShowGameDigestDialog(const std::string&) override {}
  void SetGameDigestProgress(int, int) override {}
  void SetGameDigestResult(int, const std::string&) override {}
  void AbortGameDigest() override {}
  void OnIndexAdded(bool, std::string) override {}
  void OnIndexRefreshFailed(std::string) override {}
  void ShowChunkedProgressDialog(const std::string&, u64, std::span<const int>) override {}
  void HideChunkedProgressDialog() override {}
  void SetChunkedProgress(int, u64) override {}
  void SetHostWiiSyncData(std::vector<u64>, std::string) override {}

private:
  bool LoadGame(const std::string& path)
  {
    auto file = std::make_shared<UICommon::GameFile>(path);
    if (!file->IsValid())
    {
      Fail("The requested game file is invalid");
      return false;
    }
    m_game_path = path;
    m_game_file = std::move(file);
    return true;
  }
  void Fail(const std::string& message)
  {
    m_status.store(WII_DOLPHIN_NETPLAY_FAILED, std::memory_order_release);
    PushEvent(WII_DOLPHIN_NETPLAY_EVENT_ERROR, 0, message);
  }
  void PushEvent(const uint32_t type, const int32_t code, const std::string& message)
  {
    WiiDolphinFfiNetPlayEvent event{};
    event.type = type;
    event.code = code;
    std::memcpy(event.message, message.data(), std::min(message.size(), sizeof(event.message) - 1));
    std::lock_guard lock(m_event_mutex);
    if (m_events.size() == 128)
      m_events.pop_front();
    m_events.push_back(event);
  }

  WindowSystemInfo m_wsi;
  std::string m_game_path;
  std::shared_ptr<UICommon::GameFile> m_game_file;
  std::unique_ptr<NetPlay::NetPlayServer> m_server;
  std::unique_ptr<NetPlay::NetPlayClient> m_client;
  std::atomic<bool> m_is_hosting{};
  std::atomic<int> m_status{WII_DOLPHIN_NETPLAY_STARTING};
  std::mutex m_event_mutex;
  std::deque<WiiDolphinFfiNetPlayEvent> m_events;
};

namespace
{
std::mutex s_session_mutex;
std::weak_ptr<FfiNetPlaySession> s_session;
std::atomic<bool> s_run_requested{};

std::shared_ptr<FfiNetPlaySession> GetSession()
{
  std::lock_guard lock(s_session_mutex);
  return s_session.lock();
}
}  // namespace

bool FfiNetPlayRunRequested() { return s_run_requested.load(std::memory_order_acquire); }
void FfiNetPlayRequestRun(const bool requested)
{
  s_run_requested.store(requested, std::memory_order_release);
}
std::shared_ptr<FfiNetPlaySession> FfiNetPlayCreate(const WindowSystemInfo& wsi)
{
  return std::make_shared<FfiNetPlaySession>(wsi);
}
void FfiNetPlayPublish(const std::shared_ptr<FfiNetPlaySession>& session)
{
  std::lock_guard lock(s_session_mutex);
  s_session = session;
}
void FfiNetPlayUnpublish(const std::shared_ptr<FfiNetPlaySession>& session)
{
  std::lock_guard lock(s_session_mutex);
  if (s_session.lock() == session)
    s_session.reset();
}
void FfiNetPlayShutdownSession(std::shared_ptr<FfiNetPlaySession>& session)
{
  if (!session)
    return;
  FfiNetPlayUnpublish(session);
  session->Stop();
  session.reset();
}

int32_t FfiNetPlayHost(const char* game_path, const u16 port, const char* nickname)
{
  auto session = GetSession();
  if (!session || !game_path || !*game_path || !nickname || !*nickname || port == 0)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  Core::QueueHostJob([session, path = std::string(game_path), port,
                      name = std::string(nickname)](Core::System&) mutable {
    session->Host(std::move(path), port, std::move(name));
  }, true);
  return WII_DOLPHIN_FFI_OK;
}
int32_t FfiNetPlayJoin(const char* game_path, const char* address, const u16 port,
                       const char* nickname)
{
  auto session = GetSession();
  if (!session || !game_path || !*game_path || !address || !*address || !nickname || !*nickname ||
      port == 0)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  Core::QueueHostJob([session, path = std::string(game_path), host = std::string(address), port,
                      name = std::string(nickname)](Core::System&) mutable {
    session->Join(std::move(path), std::move(host), port, std::move(name));
  }, true);
  return WII_DOLPHIN_FFI_OK;
}
int32_t FfiNetPlayStartGame()
{
  auto session = GetSession();
  if (!session)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  Core::QueueHostJob([session](Core::System&) { session->StartGame(); }, true);
  return WII_DOLPHIN_FFI_OK;
}
int32_t FfiNetPlayGetStatus()
{
  const auto session = GetSession();
  return session ? session->Status() : WII_DOLPHIN_NETPLAY_IDLE;
}
int32_t FfiNetPlayPollEvent(WiiDolphinFfiNetPlayEvent* event)
{
  if (!event)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  const auto session = GetSession();
  if (!session)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  return session->PollEvent(event) ? 1 : 0;
}
int32_t FfiNetPlayStop()
{
  auto session = GetSession();
  if (!session)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  Core::QueueHostJob([session](Core::System&) {
    session->Stop();
  }, true);
  return WII_DOLPHIN_FFI_OK;
}
