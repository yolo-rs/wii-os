// Copyright 2008 Dolphin Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later

#include "DolphinNoGUI/Platform.h"

#include <OptionParser.h>
#include <csignal>
#include <atomic>
#include <cstdio>
#include <iomanip>
#include <mutex>
#include <sstream>
#include <string>
#include <vector>

#ifdef WII_OS_DOLPHIN_FFI
#include "DolphinNoGUI/NetPlayFFI.h"
#include "wii_dolphin_ffi.h"
#endif

#ifndef _WIN32
#include <unistd.h>
#else
#include <windows.h>
#endif

#include "Common/ScopeGuard.h"
#include "Core/Boot/Boot.h"
#include "Core/BootManager.h"
#include "Core/Core.h"
#include "Core/DolphinAnalytics.h"
#include "Core/Host.h"
#include "Core/System.h"

#include "UICommon/CommandLineParse.h"
#ifdef USE_DISCORD_PRESENCE
#include "UICommon/DiscordPresence.h"
#endif
#include "UICommon/UICommon.h"

static std::unique_ptr<Platform> s_platform;
static std::atomic<Platform*> s_ffi_platform{};
static std::atomic<bool> s_ffi_session_active{};
static std::atomic<bool> s_ffi_stop_requested{};
static std::mutex s_ffi_platform_mutex;

static void signal_handler(int)
{
  constexpr char message[] = "A signal was received. A second signal will force Dolphin to stop.\n";
#ifdef _WIN32
  puts(message);
#else
  if (write(STDERR_FILENO, message, sizeof(message)) < 0)
  {
  }
#endif

  s_platform->RequestShutdown();
}

std::vector<std::string> Host_GetPreferredLocales()
{
  return {};
}

void Host_PPCSymbolsChanged()
{
}

void Host_PPCBreakpointsChanged()
{
}

bool Host_UIBlocksControllerState()
{
  return false;
}

void Host_Message(const HostMessageID id)
{
  if (id == HostMessageID::WMUserStop)
    s_platform->Stop();
}

void Host_UpdateTitle(const std::string& title)
{
  s_platform->SetTitle(title);
}

void Host_UpdateDisasmDialog()
{
}

void Host_JitCacheInvalidation()
{
}

void Host_JitProfileDataWiped()
{
}

void Host_RequestRenderWindowSize(int width, int height)
{
}

bool Host_RendererHasFocus()
{
  return s_platform->IsWindowFocused();
}

bool Host_RendererHasFullFocus()
{
  // Mouse capturing isn't implemented
  return Host_RendererHasFocus();
}

bool Host_RendererIsFullscreen()
{
  return s_platform->IsWindowFullscreen();
}

bool Host_TASInputHasFocus()
{
  return false;
}

void Host_YieldToUI()
{
}

void Host_TitleChanged()
{
#ifdef USE_DISCORD_PRESENCE
  Discord::UpdateDiscordPresence();
#endif
}

void Host_UpdateDiscordClientID(const std::string& client_id)
{
#ifdef USE_DISCORD_PRESENCE
  Discord::UpdateClientID(client_id);
#endif
}

bool Host_UpdateDiscordPresenceRaw(const std::string& details, const std::string& state,
                                   const std::string& large_image_key,
                                   const std::string& large_image_text,
                                   const std::string& small_image_key,
                                   const std::string& small_image_text,
                                   const int64_t start_timestamp, const int64_t end_timestamp,
                                   const int party_size, const int party_max)
{
#ifdef USE_DISCORD_PRESENCE
  return Discord::UpdateDiscordPresenceRaw(details, state, large_image_key, large_image_text,
                                           small_image_key, small_image_text, start_timestamp,
                                           end_timestamp, party_size, party_max);
#else
  return false;
#endif
}

std::unique_ptr<GBAHostInterface> Host_CreateGBAHost(std::weak_ptr<HW::GBA::Core> core)
{
  return nullptr;
}

static std::unique_ptr<Platform> GetPlatform(const optparse::Values& options)
{
  std::string platform_name = static_cast<const char*>(options.get("platform"));

#if HAVE_X11
  if (platform_name == "x11" || platform_name.empty())
    return Platform::CreateX11Platform();
#endif

#ifdef __linux__
  if (platform_name == "fbdev" || platform_name.empty())
    return Platform::CreateFBDevPlatform();
#endif

#ifdef _WIN32
  if (platform_name == "win32" || platform_name.empty())
    return Platform::CreateWin32Platform();
#endif
#ifdef __APPLE__
  if (platform_name == "macos" || platform_name.empty())
    return Platform::CreateMacOSPlatform();
#endif

  if (platform_name == "headless" || platform_name.empty())
    return Platform::CreateHeadlessPlatform();

  return nullptr;
}

#ifdef _WIN32
#ifndef WII_OS_DOLPHIN_FFI
#define main app_main
#endif
#endif

static int DolphinNoGUI_Main(const int argc, char* argv[])
{
  const auto parser =
      CommandLineParse::CreateParser(CommandLineParse::ParserOptions::OmitGUIOptions);
  parser->add_option("-p", "--platform")
      .action("store")
      .help("Window platform to use [%choices]")
      .choices({"headless"
#ifdef __linux__
                ,
                "fbdev"
#endif
#if HAVE_X11
                ,
                "x11"
#endif
#ifdef _WIN32
                ,
                "win32"
#endif
#ifdef __APPLE__
                ,
                "macos"
#endif
      });

  optparse::Values& options = CommandLineParse::ParseArguments(parser.get(), argc, argv);
  std::vector<std::string> args = parser->args();

  std::optional<std::string> save_state_path;
  if (options.is_set("save_state"))
  {
    save_state_path = static_cast<const char*>(options.get("save_state"));
  }

  std::unique_ptr<BootParameters> boot;
  bool game_specified = false;
  if (options.is_set("exec"))
  {
    const std::list<std::string> paths_list = options.all("exec");
    const std::vector<std::string> paths{std::make_move_iterator(std::begin(paths_list)),
                                         std::make_move_iterator(std::end(paths_list))};
    boot = BootParameters::GenerateFromFile(
        paths, BootSessionData(save_state_path, DeleteSavestateAfterBoot::No));
    game_specified = true;
  }
  else if (options.is_set("nand_title"))
  {
    const std::string hex_string = static_cast<const char*>(options.get("nand_title"));
    if (hex_string.length() != 16)
    {
      fprintf(stderr, "Invalid title ID\n");
      parser->print_help();
      return 1;
    }
    const u64 title_id = std::stoull(hex_string, nullptr, 16);
    boot = std::make_unique<BootParameters>(BootParameters::NANDTitle{title_id});
  }
  else if (args.size())
  {
    boot = BootParameters::GenerateFromFile(
        args.front(), BootSessionData(save_state_path, DeleteSavestateAfterBoot::No));
    args.erase(args.begin());
    game_specified = true;
  }
  else
  {
#ifdef WII_OS_DOLPHIN_FFI
    if (FfiNetPlayRunRequested())
      goto netplay_no_boot;
#endif
    parser->print_help();
    return 0;
  }
#ifdef WII_OS_DOLPHIN_FFI
netplay_no_boot:
#endif

  std::string user_directory;
  if (options.is_set("user"))
    user_directory = static_cast<const char*>(options.get("user"));

  s_platform = GetPlatform(options);
  Common::ScopeGuard platform_guard([] {
    std::lock_guard lock(s_ffi_platform_mutex);
    s_ffi_platform.store(nullptr, std::memory_order_release);
    s_platform.reset();
  });
  if (!s_platform || !s_platform->Init())
  {
    fprintf(stderr, "No platform found, or failed to initialize.\n");
    return 1;
  }
  {
    std::lock_guard lock(s_ffi_platform_mutex);
    s_ffi_platform.store(s_platform.get(), std::memory_order_release);
    if (s_ffi_stop_requested.load(std::memory_order_acquire))
      s_platform->RequestShutdown();
  }

  const WindowSystemInfo wsi = s_platform->GetWindowSystemInfo();

  UICommon::SetUserDirectory(user_directory);
  UICommon::Init();
  UICommon::InitControllers(wsi);

  Common::ScopeGuard ui_common_guard([] {
    UICommon::ShutdownControllers();
    UICommon::Shutdown();
  });

  if (save_state_path && !game_specified)
  {
    fprintf(stderr, "A save state cannot be loaded without specifying a game to launch.\n");
    return 1;
  }

  bool netplay_mode = false;
#ifdef WII_OS_DOLPHIN_FFI
  netplay_mode = FfiNetPlayRunRequested();
#endif
  auto core_state_changed_hook = Core::AddOnStateChangedCallback([netplay_mode](const Core::State state) {
    if (state == Core::State::Uninitialized && !netplay_mode)
      s_platform->Stop();
  });

#ifdef _WIN32
  std::signal(SIGINT, signal_handler);
  std::signal(SIGTERM, signal_handler);
#else
  // Shut down cleanly on SIGINT and SIGTERM
  struct sigaction sa;
  sa.sa_handler = signal_handler;
  sigemptyset(&sa.sa_mask);
  sa.sa_flags = SA_RESTART | SA_RESETHAND;
  sigaction(SIGINT, &sa, nullptr);
  sigaction(SIGTERM, &sa, nullptr);
#endif

  DolphinAnalytics::Instance().ReportDolphinStart("nogui");

  bool should_boot = true;
#ifdef WII_OS_DOLPHIN_FFI
  std::shared_ptr<FfiNetPlaySession> netplay_session;
  if (FfiNetPlayRunRequested())
  {
    netplay_session = FfiNetPlayCreate(wsi);
    FfiNetPlayPublish(netplay_session);
    should_boot = false;
  }
#endif
  if (should_boot && !BootManager::BootCore(Core::System::GetInstance(), std::move(boot), wsi))
  {
    fprintf(stderr, "Could not boot the specified file\n");
    return 1;
  }

#ifdef USE_DISCORD_PRESENCE
  Discord::UpdateDiscordPresence();
#endif

  s_platform->MainLoop();
#ifdef WII_OS_DOLPHIN_FFI
  if (netplay_session)
  {
    FfiNetPlayShutdownSession(netplay_session);
  }
#endif
  Core::Stop(Core::System::GetInstance());

  Core::Shutdown(Core::System::GetInstance());

  return 0;
}

#ifdef WII_OS_DOLPHIN_FFI
extern "C" WII_DOLPHIN_FFI_EXPORT uint32_t wii_dolphin_ffi_abi_version()
{
  return 1;
}

extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_run(
    const char* content_path, const char* user_dir, const uint64_t title_id,
    const uintptr_t parent_window, const char* video_backend)
{
#ifndef _WIN32
  return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
#else
  if ((title_id == 0 && (!content_path || !*content_path)) || !user_dir || !*user_dir ||
      (title_id != 0 && content_path && *content_path))
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  bool expected = false;
  if (!s_ffi_session_active.compare_exchange_strong(expected, true, std::memory_order_acq_rel))
    return WII_DOLPHIN_FFI_BUSY;

  struct SessionReset
  {
    ~SessionReset()
    {
      s_ffi_stop_requested.store(false, std::memory_order_release);
      s_ffi_session_active.store(false, std::memory_order_release);
    }
  } reset;

  try
  {
    Platform::SetWin32ParentWindow(reinterpret_cast<void*>(parent_window));
    std::vector<std::string> arguments{"wii-os", "--platform", "win32", "--user", user_dir};
    if (title_id != 0)
    {
      std::ostringstream title;
      title << std::hex << std::setfill('0') << std::setw(16) << title_id;
      arguments.emplace_back("--nand_title");
      arguments.emplace_back(title.str());
    }
    else
    {
      arguments.emplace_back("--exec");
      arguments.emplace_back(content_path);
    }
    if (video_backend && *video_backend)
    {
      arguments.emplace_back("--video_backend");
      arguments.emplace_back(video_backend);
    }
    for (const char* setting : {
             // Wii OS HD preset: 3x EFB scaling plus optional user-supplied texture replacements.
             "GFX.Settings.InternalResolution=3",
             "GFX.Settings.HiresTextures=True",
             "GFX.Settings.CacheHiresTextures=True",
             "GFX.Enhancements.MaxAnisotropy=4",
             "GFX.Settings.DumpTextures=False",
             "GFX.Settings.EnableMods=False",
             "GFX.Settings.EnableValidationLayer=False",
             "GFX.Settings.WireFrame=False",
             "GFX.Settings.ShowFPS=False",
             "GFX.Settings.ShowFTimes=False",
             "GFX.Settings.ShowVPS=False",
             "GFX.Settings.ShowVTimes=False",
             "GFX.Settings.ShowGraphs=False",
             "GFX.Settings.ShowSpeed=False",
             "GFX.Settings.LogRenderTimeToFile=False",
             "GFX.Settings.OverlayStats=False",
             "GFX.Settings.OverlayProjStats=False",
             "GFX.Settings.OverlayScissorStats=False",
             "GFX.Settings.ShowInternalResolution=False",
             "GFX.Settings.TexFmtOverlayEnable=False",
             "GFX.Settings.DumpEFBTarget=False",
             "GFX.Settings.DumpXFBTarget=False",
             "GFX.Settings.DumpFramesAsImages=False",
             "GFX.Enhancements.PostProcessingShader="})
    {
      arguments.emplace_back("--config");
      arguments.emplace_back(setting);
    }

    std::vector<char*> argv;
    argv.reserve(arguments.size());
    for (std::string& argument : arguments)
      argv.push_back(argument.data());

    const int result = DolphinNoGUI_Main(static_cast<int>(argv.size()), argv.data());
    Platform::SetWin32ParentWindow(nullptr);
    return result == 0 ? WII_DOLPHIN_FFI_OK : WII_DOLPHIN_FFI_BOOT_FAILED;
  }
  catch (...)
  {
    Platform::SetWin32ParentWindow(nullptr);
    return WII_DOLPHIN_FFI_INTERNAL_ERROR;
  }
#endif
}

extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_request_stop()
{
  s_ffi_stop_requested.store(true, std::memory_order_release);
  std::lock_guard lock(s_ffi_platform_mutex);
  Platform* platform = s_ffi_platform.load(std::memory_order_acquire);
  if (!platform)
    return s_ffi_session_active.load(std::memory_order_acquire) ? WII_DOLPHIN_FFI_OK
                                                                : WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  platform->RequestShutdown();
  return WII_DOLPHIN_FFI_OK;
}

extern "C" WII_DOLPHIN_FFI_EXPORT uintptr_t wii_dolphin_ffi_render_window()
{
  std::lock_guard lock(s_ffi_platform_mutex);
  Platform* platform = s_ffi_platform.load(std::memory_order_acquire);
  if (!platform)
    return 0;
  return reinterpret_cast<uintptr_t>(platform->GetWindowSystemInfo().render_window);
}

extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_run(
    const char* user_dir, const uintptr_t parent_window, const char* video_backend)
{
#ifndef _WIN32
  return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
#else
  if (!user_dir || !*user_dir)
    return WII_DOLPHIN_FFI_INVALID_ARGUMENT;
  bool expected = false;
  if (!s_ffi_session_active.compare_exchange_strong(expected, true, std::memory_order_acq_rel))
    return WII_DOLPHIN_FFI_BUSY;
  struct SessionReset
  {
    ~SessionReset()
    {
      FfiNetPlayRequestRun(false);
      s_ffi_stop_requested.store(false, std::memory_order_release);
      s_ffi_session_active.store(false, std::memory_order_release);
    }
  } reset;
  try
  {
    Platform::SetWin32ParentWindow(reinterpret_cast<void*>(parent_window));
    FfiNetPlayRequestRun(true);
    std::vector<std::string> arguments{"wii-os", "--platform", "win32", "--user", user_dir};
    if (video_backend && *video_backend)
    {
      arguments.emplace_back("--video_backend");
      arguments.emplace_back(video_backend);
    }
    for (const char* setting : {"GFX.Settings.ShowFPS=False", "GFX.Settings.ShowFTimes=False",
                                "GFX.Settings.ShowVPS=False", "GFX.Settings.ShowVTimes=False",
                                "Main.NetPlay.SyncSaves=False", "Main.NetPlay.WriteSaveData=False",
                                "Main.NetPlay.SyncCodes=False"})
    {
      arguments.emplace_back("--config");
      arguments.emplace_back(setting);
    }
    std::vector<char*> argv;
    argv.reserve(arguments.size());
    for (std::string& argument : arguments)
      argv.push_back(argument.data());
    const int result = DolphinNoGUI_Main(static_cast<int>(argv.size()), argv.data());
    Platform::SetWin32ParentWindow(nullptr);
    return result == 0 ? WII_DOLPHIN_FFI_OK : WII_DOLPHIN_FFI_BOOT_FAILED;
  }
  catch (...)
  {
    Platform::SetWin32ParentWindow(nullptr);
    return WII_DOLPHIN_FFI_INTERNAL_ERROR;
  }
#endif
}

extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_host(
    const char* game_path, const uint16_t port, const char* nickname)
{
  return FfiNetPlayHost(game_path, port, nickname);
}
extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_join(
    const char* game_path, const char* address, const uint16_t port, const char* nickname)
{
  return FfiNetPlayJoin(game_path, address, port, nickname);
}
extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_start_game()
{
  return FfiNetPlayStartGame();
}
extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_status()
{
  return FfiNetPlayGetStatus();
}
extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_poll_event(
    WiiDolphinFfiNetPlayEvent* event)
{
  return FfiNetPlayPollEvent(event);
}
extern "C" WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_stop()
{
  return FfiNetPlayStop();
}
#else
int main(const int argc, char* argv[])
{
  return DolphinNoGUI_Main(argc, argv);
}

#ifdef _WIN32
int wmain(int, wchar_t*[], wchar_t*[])
{
  std::vector<std::string> args = Common::CommandLineToUtf8Argv(GetCommandLineW());
  const int argc = static_cast<int>(args.size());
  std::vector<char*> argv(args.size());
  for (size_t i = 0; i < args.size(); ++i)
    argv[i] = args[i].data();

  return main(argc, argv.data());
}

#undef main
#endif
#endif
