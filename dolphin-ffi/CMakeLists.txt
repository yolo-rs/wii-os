# Copyright 2026 Wii OS contributors
# SPDX-License-Identifier: GPL-2.0-or-later

if(NOT WIN32)
  message(FATAL_ERROR "wii-dolphin-ffi currently requires the Windows Dolphin host")
endif()

add_library(wii_dolphin_ffi SHARED
  "${CMAKE_SOURCE_DIR}/Source/Core/DolphinNoGUI/NetPlayFFI.cpp"
  "${CMAKE_SOURCE_DIR}/Source/Core/DolphinNoGUI/PlatformHeadless.cpp"
  "${CMAKE_SOURCE_DIR}/Source/Core/DolphinNoGUI/MainNoGUI.cpp"
  "${CMAKE_SOURCE_DIR}/Source/Core/DolphinNoGUI/Platform.cpp"
  "${CMAKE_SOURCE_DIR}/Source/Core/DolphinNoGUI/PlatformWin32.cpp"
)

target_compile_definitions(wii_dolphin_ffi PRIVATE WII_OS_DOLPHIN_FFI)
target_include_directories(wii_dolphin_ffi PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/include"
  "${CMAKE_SOURCE_DIR}/Source/Core"
)
target_link_libraries(wii_dolphin_ffi PRIVATE
  core
  uicommon
  cpp-optparse
  dwmapi.lib
)
if(MSVC)
  target_link_libraries(wii_dolphin_ffi PRIVATE use_pch)
endif()
set_target_properties(wii_dolphin_ffi PROPERTIES
  OUTPUT_NAME "wii_dolphin_ffi"
  RUNTIME_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/Binaries>"
  LIBRARY_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/Binaries>"
  ARCHIVE_OUTPUT_DIRECTORY "$<1:${CMAKE_BINARY_DIR}/Binaries>"
)
