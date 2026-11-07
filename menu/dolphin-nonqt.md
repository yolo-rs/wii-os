# Dolphin without Qt: source and build targets

Use the patched Dolphin source snapshot at `src/dolphin`, recorded in
`source-roots.json`. Its originating commit is pinned in
`src/dolphin/SOURCE-REVISION.txt`; the no-Qt host and its FFI are built from
this product-owned source tree.

## Existing non-Qt targets

| Target | Source | Use |
|---|---|---|
| `dolphin-tool` | `Source/Core/DolphinTool/` | Console utility: `convert`, `verify`, `header`, and `extract` for disc images |
| `dolphin-nogui` | `Source/Core/DolphinNoGUI/` | Emulator frontend without DolphinQt; platform may still create a Win32/X11 window unless headless mode is selected |
| `core`, `discio`, `uicommon`, `videocommon`, `videobackends` | `Source/Core/` | Runtime libraries used by Dolphin frontends and tools; retain Dolphin's own CMake dependency graph |

Top-level CMake declares `ENABLE_CLI_TOOL=ON` by default on non-Android and
`ENABLE_NOGUI=OFF`, `ENABLE_QT=ON`, `ENABLE_HEADLESS=OFF` by default. For a
Windows native frontend without Qt, configure an out-of-tree CMake build with
MSVC and:

```powershell
$dolphinSource = (Get-Content source-roots.json -Raw | ConvertFrom-Json).repositories |
  Where-Object name -eq 'wii-os/src/dolphin' | Select-Object -First 1
$dolphinSource = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot $dolphinSource.path))
cmake -S $dolphinSource -B userland/build/targets/dolphin-nonqt `
  -G "Visual Studio 18 2026" -A x64 `
  -DENABLE_QT=OFF -DENABLE_NOGUI=ON -DENABLE_CLI_TOOL=ON -DENABLE_HEADLESS=OFF
cmake --build userland/build/targets/dolphin-nonqt --config Release `
  --target dolphin-tool dolphin-nogui
```

The `dolphin-nogui` name means no Qt frontend; on Windows it can use
`PlatformWin32.cpp` and display an emulator window. For a truly headless
automation lane, set `-DENABLE_HEADLESS=ON` and use Dolphin's headless platform
selection. Do not assume that this mode provides the interactive native
GameScope UI. The current checkout's `DolphinNoGUI/CMakeLists.txt` compiles the
headless platform implementation alongside platform-specific choices.

The no-Qt frontend accepts `--user <profile> --nand_title <16-hex-title-id>`
for a title already installed in that profile's `Wii/title` tree. Wii OS models
this as `contentKind: "wii-nand-title"`; launch readiness checks the installed
title TMD before starting Dolphin. This supports an actual installed System
Menu and channels while leaving IOS, NAND, saves, controller emulation, and title
execution to Dolphin. File-based WAD boot remains a separate `--exec <wad>`
route. The GameScope shell can own the library/settings flow, but removing
Dolphin's emulation core would require replacing CPU, GPU, audio, IOS, NAND,
input, save, and NetPlay implementations. Gamescope itself is a compositor,
not a Wii emulation API.

Set `WII_OS_DOLPHIN_VIDEO_BACKEND` to Dolphin's `D3D`, `D3D12`, `Vulkan`,
`OGL`, or `Software Renderer` backend names. Wii OS also accepts the aliases
`OpenGL` and `Software` and maps them to Dolphin's identifiers. This is an
environment override; the native shell does not yet expose a persisted
graphics settings page.

Wii OS launches explicitly disable Dolphin's wireframe, graphics mods,
validation layers, post-processing shaders, performance overlays, texture/EFB
dumps, and render-time logging. The setup launcher also does not open Dolphin's
logger or debugger window. These command-line overrides leave a user's normal
Dolphin profile intact while keeping the Wii OS launch path free of diagnostic
and experimental rendering modes.

`dolphin-tool` is the suitable first CLI adapter for the unified Wii pipeline:
call its own help and supported commands rather than reimplementing Dolphin's
disc parser. `Boot.cpp` recognizes `.wad` and routes it to the Wii WAD boot
path in the emulator frontend; `dolphin-tool`'s documented command set does
not promise WAD installation or System Menu boot. Keep those as separate
commands/contracts.

## Wii OS integration

The Windows portable menu additionally builds `wii_dolphin_ffi.dll` from the
same C++ source and links the Rust `wii-dolphin-ffi` session wrapper. The ABI
starts one DolphinNoGUI host loop on a dedicated thread, creates a borderless
child HWND under the Wii OS window, and forwards stop/window queries through a
fixed C ABI. Dolphin's singleton core permits only one active session in a
process. The standalone no-Qt executable remains available to the HTTP/MCP
service and non-embedded platform routes.

The `wii` CLI should expose an explicit `dolphin tool` namespace and delegate
to a configured Dolphin build. It can wrap `header`, `verify`, `extract`, and
`convert`; it should keep arguments and exit codes intact and record the
binary version and report path. `wii run --engine dolphin --wad <user-path>`
can launch the emulator frontend only after recording an operator-provided
WAD/NAND path and relevant boot settings. Neither CLI routing nor building
Dolphin provides copyrighted WADs, keys, NAND data, or menu assets.

On Linux, the `dolphin-nogui` frontend is separate from GTK/Wayland shell
integration. On the browser, Dolphin has no WebGPU/WASM target in this
checkout; Gecko is the existing browser emulator. Dolphin's technical report
says a playable web port would need a new PPC-to-WASM JIT and a WebGL video
backend, while AOT alone cannot handle runtime-generated or modified game
code. Keep Dolphin native to Windows/Linux and do not advertise it as a
browser backend until those components exist.
