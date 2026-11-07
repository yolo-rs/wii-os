# Windows 11 native menu and GameScope architecture

## Current source facts

- Gecko is the product's cross-platform Wii/GameCube Rust runtime in the root Cargo workspace. Its native app uses `iced`, `backend-wgpu`, and `wgpu`; it does not implement the Wii Menu launcher as its frontend. See [`Gecko documentation`](../docs/gecko/README.md) and [`crates/gecko-app/Cargo.toml`](../crates/gecko-app/Cargo.toml).
- Gecko's browser renderer explicitly selects `wgpu::Backends::BROWSER_WEBGPU`. Its native developer tools set `Backends::all().with_env()`; this permits backend selection via `WGPU_BACKEND=dx12` where those instance constructors are used. Do not infer that every Gecko GUI renderer selects D3D12 until the actual app is run and its adapter/backend logged.
- Dolphin already has a native Windows D3D12 video backend: `Source/Core/VideoBackends/CMakeLists.txt` adds `D3D12` under `WIN32`, and the backend sources live in `Source/Core/VideoBackends/D3D12`. Dolphin's boot code is the existing realistic Windows route for running an operator-provided Wii Menu WAD and NAND; it is separate from drawing a launcher UI.
- Wii.JS is a browser menu reproduction/prototype. The Wii IPL repository is an incomplete C decompilation which requires an operator-provided WAD and can produce a `main.dol`; neither is a Windows-native menu frontend nor proof of pixel-perfect parity. The real System Menu and its assets remain operator inputs and are not bundled here.
- Gecko and Dolphin carry their upstream licenses and notices in `crates/gecko-core/LICENSE` and `src/dolphin/COPYING`. They share one product tree while retaining their distinct code and license obligations.

## Current coordinator implementation

- `wii-os/apps/menu-native` is a Rust `winit` + `wgpu` window, independent of WinUI. Its Windows backend selection uses `wgpu::Backends::DX12` by default and accepts `WII_OS_GPU_BACKEND=dx12|vulkan|gl|auto` for an explicit development selection.
- On 2026-10-30, a release build was run with `WII_OS_GPU_BACKEND=dx12`; the renderer logged `adapter=NVIDIA GeForce RTX 4070 backend=Dx12 device=DiscreteGpu`. This proves adapter initialization and presentation on this host, not coverage across other GPUs or Windows versions. Direct3D feature-level availability is a runtime device capability; query it rather than assuming a hardware feature from the API name ([Microsoft Learn: hardware feature levels](https://learn.microsoft.com/windows/win32/direct3d12/hardware-feature-levels)).
- The native scene now textures the channel cards, empty slots, footer and controls from the checked-in frontend artwork. Channel images and text baked into those images render, but the native app does not reproduce the web shell's audio, transitions, settings view, date/localization, or complete focus/controller model. No physical-console reference diff is available.
- Pressing Enter on a selected channel resolves only relative catalogued `.wad`/`.dol` paths contained under the workspace, requires an existing `WII_OS_DOLPHIN_EXE`, and launches through a configured Dolphin user directory. Catalog entries can also boot already installed NAND titles, including the System Menu, with `--nand_title`; the installed TMD is checked before launch. Wii OS does not install WADs or modify NAND content, and Dolphin remains a separate emulation process.
- The reproducible compiler route is `pipeline.ps1 -Profile menu-native -Target windows-x64`; the GPU backend/runtime observation is a separate manual qualification. Wgpu’s Windows D3D12 backend and browser WebGPU backend are distinct selections ([wgpu 29 API and backend docs](https://github.com/gfx-rs/wgpu/tree/wgpu-v29.0.1)).

## Recommended Windows composition

Use one process and one owner per concern:

1. **GameScope shell (C++/WinUI 3):** windowing, titlebar, file pickers, settings, controller/device pages, library and accessible controls. Keep WinUI responsive and avoid doing emulation work on the UI thread.
2. **Menu renderer (Rust, winit + wgpu):** one reusable menu scene/state model and a wgpu surface. On Windows, compile the `dx12` backend and request/select `Backends::DX12` (or expose an explicit `auto|dx12|vulkan` setting). `WGPU_BACKEND=dx12` is useful for development diagnostics, not a substitute for product configuration or runtime adapter logging. On the web target use Gecko's separate browser-WebGPU instance.
3. **Emulation runtime:** DolphinNoGUI is the native no-Qt Wii runtime for WADs, installed NAND titles, controllers, saves and NetPlay; Gecko supplies the browser/WebGPU runtime for its supported boot paths. The GameScope shell launches/switches runtime sessions through a small versioned local IPC contract and does not embed emulator internals into XAML. Gamescope by Valve can host/composite Linux applications but does not provide Wii emulation APIs.
4. **Wii channel content:** resolve paths and user-owned saves/NAND through a platform storage service. Keep WAD installation, ticket/TMD validation, IOS/NAND setup, and title launch explicit. A WAD is not a self-contained browser boot image.

Avoid making both WinUI and winit own competing top-level windows. Two implementation choices should be prototyped before committing the host boundary:

- **Preferred first vertical slice:** WinUI 3 owns the application window; a single C++/DirectX 12 renderer owns the menu surface, with WinUI controls composited above it using supported Windows composition interop. This meets the requested native C++ DirectX 12 shell without an undocumented wgpu/DXGI texture-sharing bridge.
- **Alternative shared renderer:** host a winit/wgpu native child surface in a Win32/WinUI shell and expose the menu through a Rust/C ABI. This is the better code-sharing path with Linux and WebGPU, but HWND parenting, DPI, resize, focus, accessibility overlays, device-loss and swapchain lifetime need a small native spike. It does not make WinUI controls into the wgpu surface.

Do not build a renderer that owns both a wgpu DX12 swapchain and a second DirectX 12 swapchain for the same menu. If low-level D3D12 interop is eventually necessary, define who owns the device, queue, fences, resource states, resize/device-loss recovery, and presentation; keep that interop behind one narrow C ABI.

## Cross-platform boundary

| Layer | Windows 11 | Linux | Browser |
|---|---|---|---|
| Menu state, channel model, input mapping | shared Rust core | shared Rust core | shared Rust/WASM core where Web APIs allow |
| Native menu rendering | wgpu/DX12 first; Vulkan optional | wgpu/Vulkan first; Wayland surface | browser WebGPU only |
| OS shell | WinUI 3 / C++ host | separate GTK4 or compositor/plugin integration | HTML/CSS host |
| Emulation | Gecko or Dolphin native | Gecko or Dolphin native | Gecko WASM, with current GameCube DOL/IPL scope |
| Actual Wii System Menu boot | Dolphin WAD + configured NAND; Gecko paths require feature work | same runtime boundary | not implemented in Gecko web today |

Linux now has two hosts over the same core: the winit/wgpu renderer and an initial GTK4 app host. The GTK host builds in the unified Linux profile; embedding the shared wgpu scene, persistent settings, controller focus mapping, and compositor runtime qualification remain follow-up work. A Hyprland layer-shell plugin must remain optional and must not become a dependency of the portable renderer or core.

## Pixel-fidelity qualification

“Pixel perfect” is a measured acceptance target, not a code architecture claim. For every reference state, retain an authorized reference screenshot and record region/locale, aspect ratio, output mode, animation timestamp, DPI/scale, selected channel, cursor position, and viewport. Compare the same state and size, report pixel/edge differences, then validate input and animation behavior separately. Do not claim parity using Wii.JS alone: it is explicitly a work-in-progress reproduction and does not prove how the executable System Menu renders. Do not redistribute copyrighted System Menu assets or title content; use user-supplied authorized inputs and generated or independently licensed assets for the launcher prototype.

## First implementation milestones

1. Lock reference states and rights/provenance for the material used in comparisons.
2. Define shared `MenuState`, `Channel`, `InputEvent`, `UserStorage` and runtime-launch interfaces with deterministic sample metadata.
3. Extend the existing 4:3 wgpu/D3D12 menu scene to reference-driven channel states and transitions; preserve aspect ratio and qualify integer scaling on the output modes that support it.
4. Extend the browser WebGPU scene from channel-icon textures to the full menu shell and compare captures at the same virtual resolution.
5. Keep the winit/wgpu Windows slice as the current native baseline: D3D12 adapter selection is verified on hardware; WinUI/DXGI shell integration and visual comparison remain open.
6. Keep advancing the GTK4/Wayland host: the shared core and Dolphin launch adapter are wired, while the shared WGPU scene and controller-to-focus path still need implementation and compositor qualification.
7. Continue real WAD/NAND boot through Dolphin with explicit paths, launch logs, and no included title files. A local PAL 4.3E profile reached the first-use screen; full channel behavior and other regions still need qualification. Treat Gecko Wii-mode/WAD installation as a separate capability project.

## Source-backed API references

- [WinUI 3 overview (Microsoft Learn)](https://learn.microsoft.com/windows/apps/winui/winui3/): native Windows desktop UI framework, C++/XAML support and high-DPI input.
- [DirectX and XAML interop (Microsoft Learn)](https://learn.microsoft.com/windows/uwp/gaming/directx-and-xaml-interop#swapchainpanel-and-gaming): describes an explicitly managed DirectX swapchain with XAML overlays for real-time/game rendering. Validate the selected interop surface against the chosen Windows App SDK/WinUI 3 version before implementation.
- [Dolphin D3D12 backend CMake](../src/dolphin/Source/Core/VideoBackends/CMakeLists.txt) and [backend source](../src/dolphin/Source/Core/VideoBackends/D3D12/CMakeLists.txt).
- [Gecko app dependencies](../crates/gecko-app/Cargo.toml) and [web backend selection](../crates/gecko-web/src/lib.rs).
- [wgpu 29 docs via Context7](https://github.com/gfx-rs/wgpu/tree/wgpu-v29.0.1): documents native DX12 and the `WGPU_BACKEND` variable. The checkout lockfile is authoritative for the exact patch version.
- [winit 0.30 API migration](https://github.com/rust-windowing/winit/blob/master/winit/src/changelog/v0.30.md): the event-loop-owned `ApplicationHandler`/`ActiveEventLoop::create_window` model used by current Gecko code.
