// Copyright 2026 Wii OS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <stdint.h>

#if defined(_WIN32)
#define WII_DOLPHIN_FFI_EXPORT __declspec(dllexport)
#else
#define WII_DOLPHIN_FFI_EXPORT __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

enum WiiDolphinFfiStatus
{
  WII_DOLPHIN_FFI_OK = 0,
  WII_DOLPHIN_FFI_BOOT_FAILED = 1,
  WII_DOLPHIN_FFI_BUSY = 2,
  WII_DOLPHIN_FFI_INVALID_ARGUMENT = 3,
  WII_DOLPHIN_FFI_INTERNAL_ERROR = 4,
};

enum WiiDolphinFfiNetPlayStatus
{
  WII_DOLPHIN_NETPLAY_IDLE = 0,
  WII_DOLPHIN_NETPLAY_STARTING = 1,
  WII_DOLPHIN_NETPLAY_LOBBY = 2,
  WII_DOLPHIN_NETPLAY_RUNNING = 3,
  WII_DOLPHIN_NETPLAY_FAILED = 4,
  WII_DOLPHIN_NETPLAY_STOPPING = 5,
  WII_DOLPHIN_NETPLAY_STOPPED = 6,
};

enum WiiDolphinFfiNetPlayEventType
{
  WII_DOLPHIN_NETPLAY_EVENT_NONE = 0,
  WII_DOLPHIN_NETPLAY_EVENT_READY = 1,
  WII_DOLPHIN_NETPLAY_EVENT_HOSTED = 2,
  WII_DOLPHIN_NETPLAY_EVENT_JOINED = 3,
  WII_DOLPHIN_NETPLAY_EVENT_PLAYER_JOINED = 4,
  WII_DOLPHIN_NETPLAY_EVENT_PLAYER_LEFT = 5,
  WII_DOLPHIN_NETPLAY_EVENT_GAME_STARTED = 6,
  WII_DOLPHIN_NETPLAY_EVENT_GAME_STOPPED = 7,
  WII_DOLPHIN_NETPLAY_EVENT_ERROR = 8,
  WII_DOLPHIN_NETPLAY_EVENT_CONNECTION_LOST = 9,
  WII_DOLPHIN_NETPLAY_EVENT_CHAT = 10,
};

struct WiiDolphinFfiNetPlayEvent
{
  uint32_t type;
  int32_t code;
  char message[256];
};

WII_DOLPHIN_FFI_EXPORT uint32_t wii_dolphin_ffi_abi_version(void);

// Runs one Dolphin host session on the calling thread until the title exits.
// Strings are borrowed only for this call and must be UTF-8. title_id is zero
// for a file boot; otherwise it is a Wii title ID already installed in user_dir.
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_run(const char* content_path,
                                                  const char* user_dir,
                                                  uint64_t title_id,
                                                  uintptr_t parent_window,
                                                  const char* video_backend);

// Requests a graceful stop for the single active session. Safe from another thread.
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_request_stop(void);

// Returns the native render HWND while a session is active, or zero otherwise.
WII_DOLPHIN_FFI_EXPORT uintptr_t wii_dolphin_ffi_render_window(void);

// Runs the Dolphin host without pre-booting a title. Use this on a dedicated
// host thread, then issue direct LAN host/join/start commands and poll events.
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_run(const char* user_dir,
                                                         uintptr_t parent_window,
                                                         const char* video_backend);
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_host(const char* game_path,
                                                          uint16_t port,
                                                          const char* nickname);
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_join(const char* game_path,
                                                          const char* address,
                                                          uint16_t port,
                                                          const char* nickname);
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_start_game(void);
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_status(void);
// Returns 1 and fills event when available, 0 when empty, or a negative FFI status.
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_poll_event(
    struct WiiDolphinFfiNetPlayEvent* event);
WII_DOLPHIN_FFI_EXPORT int32_t wii_dolphin_ffi_netplay_stop(void);

#ifdef __cplusplus
}
#endif
