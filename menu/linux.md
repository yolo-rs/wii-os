# Linux menu host: implementation contract

## What exists in this checkout

- `crates/gecko-app` is the native Gecko player and already has a Wii
  launch path for a DOL and Wii disc/apploader, plus host-backed `title/` and
  `shared2/` storage. Its window/UI is currently Iced; its Linux dependencies
  include Wayland and X11 client support. This is an emulator/player, not a
  GTK4 Wii Menu implementation.
- `src/dolphin` is the Qt-free emulator host. Its boot layer recognizes
  WAD input, so the Linux path for an operator-supplied authorized System Menu
  WAD/NAND should initially launch through Dolphin. This does not make a WAD a
  standalone Linux app and does not establish pixel parity by itself.
- `sources/Wii.JS` is a static browser menu reproduction. It is not native GTK
  and does not execute Nintendo's System Menu binary.
- `sources/smithay` is compositor implementation code; `sources/wayland-rs`
  is a set of Wayland protocol/client/server crates. Neither is an app shell
  or a ready-made Wii menu plugin. Do not embed or fork either to render the
  launcher.
- `wii-os/apps/menu-native` already builds a normal Wayland/X11 client with
  `winit` and `wgpu`; Linux defaults to Vulkan plus GL, and
  `WII_OS_GPU_BACKEND=vulkan|gl|auto` selects a backend. This is a standalone
  window, not GTK4 and not a Hyprland extension. Linux binaries require a Linux
  host; no Linux GPU/window runtime or screenshot qualification is
  recorded yet.
- `wii-os/apps/menu-gtk` is a GTK4 Linux host behind the `gtk-host` Cargo
  feature. It consumes the same schema-v1 catalog, `MenuModel`, and
  `wii-menu-runtime` Dolphin launch adapter as the wgpu host. GTK4 is
  Linux-target-only and is not pulled into Windows builds. Its current tiles
  use native GTK controls; it does not yet embed the wgpu scene canvas.
- `gtk4-layer-shell` is not installed or linked. Existing Smithay and
  `wayland-rs` checkouts do not provide that application library.

The winit/wgpu renderer and GTK4 application are two native hosts over the
same state and launch contract. The GTK4 host owns the accessible native tile
controls and ordinary desktop lifecycle; winit/wgpu owns its separate scene
window. Keep future layer-shell integration behind a separate feature and
protocol probe; Smithay is a compositor framework, not an application plugin
SDK. Verify display presentation on target compositors before describing it
as supported.

## Linux composition and remaining work

1. `apps/menu-gtk` is the GTK4 launcher host (Rust `gtk4-rs`). It owns the
   application window, native channel controls, keyboard focus, and desktop
   theme integration while keeping catalog/model ownership independent of GTK.
   Settings persistence, controller-to-focus mapping and desktop accessibility
   verification are not implemented yet.
2. Build a shared menu presentation layer for the center content, channel grid,
   pointer, and transitions. Prefer the existing wgpu renderer only after it
   is factored behind a reusable surface contract; Gecko currently binds its
   renderer to emulator framebuffer output and does not expose a Wii Menu
   scene renderer. The current GTK host uses native controls and the wgpu
   renderer uses a separate winit window. GTK4 remains responsible for
   controls and accessible labels. Avoid a second compositor.
3. Offer a GTK4 theme/native mode as the default Linux presentation. Add an
   optional Wayland layer-shell plugin for a desktop shell/panel mode. Probe
   `gtk_layer_is_supported()` at runtime and expose that mode only when the
   compositor advertises `zwlr_layer_shell_v1`; a normal GTK4 window remains
   available on X11, GNOME Wayland, or unsupported compositors. The layer-shell
   library documents wlroots compositors (the family used by Hyprland), Smithay
   compositors and KDE Wayland as supported, and GNOME Wayland/X11 as
   unsupported. Treat protocol detection as authoritative rather than
   hard-coding a Hyprland name.
4. Keep emulation in the Gecko or Dolphin process boundary. The launcher sends
   a typed launch request (`title id`, authorized content paths, NAND root,
   preferred backend) and receives status/progress; it must not interpret WAD
   contents in its UI layer. Persist saves and NAND data in the user's data
   directory, and keep game images, BIOS, WADs and dumps in separately
   configured user directories, never bundled into the app package.

GTK4 is a shell/toolkit and CSS is a theme mechanism; neither yields a
pixel-perfect Wii rendering automatically. A fidelity claim needs a documented
reference capture, fixed 4:3 content region and scaling/filter policy, correct
font/texture provenance, animation timings, sound behavior, input navigation,
and visual-diff captures at the same state and resolution. Keep proprietary
Nintendo assets and title contents external and operator-provided.

## Linux boundaries and staged deliverables

| Stage | Deliverable | Exit evidence |
|---|---|---|
| L0 | Launch Gecko/Dolphin from the shared manifest and directory layout | Authorized sample DOL/disc/WAD is recognized; errors name missing tool/content clearly |
| L1 | GTK4 host with shared catalog/model, native tiles and launch adapter | Build passes; WSLg launch observed. Wayland/X11 desktop, screen-reader focus, controller mapping and persisted settings still need qualification |
| L2 | Shared menu canvas with Wii-like fixed-ratio layout and navigation | Reference/implementation screenshots and behavior matrix; no parity claim before diff |
| L3 | Optional GTK4 layer-shell plugin | Runtime protocol probe; supported under Hyprland/wlroots, fallback under unsupported Wayland/X11 |
| L4 | Real System Menu title boot | User-supplied WAD installed/loaded with compatible IOS/NAND semantics through an emulator; boot evidence per region |

L0 can use the existing Gecko executable for DOL/disc and Dolphin's WAD boot
for System Menu experiments. L1 now has an initial implementation in
`apps/menu-gtk`: a GTK4 window reads the shared schema-v1 catalog and
`MenuModel`, presents native channel tiles and settings controls, and dispatches
the same validated launch effect through `wii-menu-runtime` as the wgpu host.
`./pipeline.ps1 -Profile menu-native -Target linux-x64` builds the GTK host in
Linux; `build-menu-gtk-linux.sh` checks `pkg-config gtk4` and writes its
binary under the Linux target directory. Linux process startup was observed;
desktop Wayland/X11, accessibility, controller and persisted-settings
qualification remains outstanding. This GTK host does not embed the wgpu Wii
scene canvas, so Wii visual parity is not claimed. L2-L3 remain incomplete.
L4 is an emulator capability and operator-input problem, not a GTK feature.
Native Linux rendering APIs are
owned by wgpu/Dolphin backends; don't route Linux through Direct3D 12.

## Toolchain and shared pipeline requirements

- Build the GTK4 host on a native Linux host with its GTK4 development package,
  `pkg-config`, Rust/Cargo and the locked crate graph; declare GTK4/GObject
  runtime libraries as package dependencies rather than copying them into a
  custom sysroot.
- Build the optional layer-shell feature only where `gtk4-layer-shell` and
  its C system library are present. Keep it behind a Cargo feature and report
  a skipped optional integration if not present.
- Linux Gecko uses the existing locked Cargo workspace; emulator rendering is
  wgpu and backend availability depends on the runtime driver. Record selected
  adapter/backend in the run report. A compile alone does not prove GPU
  initialization or present.
- Package launcher configuration under the app config directory; saves and
  emulator NAND under app data; caches/thumbnails under cache. Never put user
  ROMs or saves in `/usr` or the read-only app bundle. The central manifest
  resolves user-configurable directories and permissions before launch.
- The `menu-native -Target linux-x64` pipeline calls a dedicated GTK4 build
  script; keep optional `gtk4-layer-shell` separate so base Linux builds do not depend on
  Hyprland, Smithay, or a compositor dev checkout.

## References

- [gtk4-rs application, CSS and accessibility documentation](https://github.com/gtk-rs/gtk4-rs)
- [gtk4-layer-shell supported compositors and protocol detection](https://github.com/wmww/gtk4-layer-shell)
- [wgpu 29 backend documentation](https://github.com/gfx-rs/wgpu/tree/wgpu-v29.0.1)
- Local implementation evidence: `crates/gecko-app/Cargo.toml`,
  `crates/gecko-app/src/player/state.rs`,
  `crates/gecko-core/src/wii.rs`,
  `src/dolphin/Source/Core/Core/Boot/Boot.cpp`,
  `src/dolphin/Source/Core/VideoBackends/D3D12/`, and
  `sources/Wii.JS/README.md`.
