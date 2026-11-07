// Copyright 2026 Wii OS contributors
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <memory>

#include "Common/CommonTypes.h"
#include "Common/WindowSystemInfo.h"
#include "wii_dolphin_ffi.h"

namespace NetPlay
{
class NetPlayUI;
}

class FfiNetPlaySession;

bool FfiNetPlayRunRequested();
std::shared_ptr<FfiNetPlaySession> FfiNetPlayCreate(const WindowSystemInfo& wsi);
void FfiNetPlayPublish(const std::shared_ptr<FfiNetPlaySession>& session);
void FfiNetPlayUnpublish(const std::shared_ptr<FfiNetPlaySession>& session);
void FfiNetPlayShutdownSession(std::shared_ptr<FfiNetPlaySession>& session);
void FfiNetPlayRequestRun(bool requested);

int32_t FfiNetPlayHost(const char* game_path, u16 port, const char* nickname);
int32_t FfiNetPlayJoin(const char* game_path, const char* address, u16 port,
                       const char* nickname);
int32_t FfiNetPlayStartGame();
int32_t FfiNetPlayGetStatus();
int32_t FfiNetPlayPollEvent(struct WiiDolphinFfiNetPlayEvent* event);
int32_t FfiNetPlayStop();
