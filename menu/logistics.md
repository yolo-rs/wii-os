# Shared source, data, and build logistics

## Product boundary

Treat Wii OS as one multi-target product with one menu contract and platform
hosts. Dolphin's C++ source lives under `src/dolphin`; Gecko's extracted Rust
crates live directly under `crates/gecko-*` in the root Cargo workspace. Both
retain their license notices. Product builds do not select sibling emulator
checkouts or carry a duplicate Dolphin tree inside Gecko.

The shared product layer should own only the parts that have a meaningful
cross-target contract:

| Shared contract | Platform implementation |
|---|---|
| Versioned channel/menu manifest, channel identity, ordering, state and user settings | Web storage, Windows app data, Linux XDG config/data |
| Input action vocabulary: focus, select, back, page, launch, pointer and controller | DOM/keyboard/gamepad, WinUI/controller, GTK4/Wayland/controller |
| Fixed Wii composition, scaling/filter policy, animation durations and screen-state identifiers | WebGPU canvas, native wgpu surface, Linux GTK host/canvas |
| Typed launch request and result; paths, hashes, region/title metadata and provenance | Gecko/Dolphin adapters; never parse WADs in presentation code |
| Asset index with source, license/provenance, region, hash and intended use | User-provided/private assets or independently licensed product assets |

Wii.JS now consumes the shared versioned catalog and menu-core state engine
through a wasm-bindgen bridge staged by the Web/WASM profile. Its remaining
gaps are in [recon.md](recon.md) and [features.csv](features.csv).
Windows and Linux host boundaries are in [windows.md](windows.md) and
[linux.md](linux.md). These documents are implementation contracts, not
evidence that the hosts or a real System Menu boot already exist.

## Source tree and ownership

```text
C:\src\wii\wii-os\
  src/dolphin/              patched Dolphin C++ engine and Qt-free host
  crates/gecko-*/           extracted Gecko crates in the root Cargo workspace
  crates/common/            shared Gecko instruction specifications
  assets/gecko/              licensed Gecko HLE IPL input
  dolphin-ffi/              Rust-safe ABI and Dolphin session owner
  crates/                   menu model, launch runtime and Web binding
  apps/                     CLI, server, renderer and native hosts
  frontend/                 web entry point, artwork, animation and sound
  pipeline.catalog.json     canonical in-product build map
  userland/build/reports/   logs, versions, hashes and staged deliverables
  userland/build/targets/   compiler outputs
```

The shared state/action implementation is
[`crates/menu-core/`](../crates/menu-core/src/lib.rs), a Rust crate checked for the active
host and compiled to `wasm32-unknown-unknown` through [`crates/menu-web/`](../crates/menu-web).
The CLI validates and loads the same catalog; Wii.JS consumes the WASM state
engine for web focus, preview and launch effects. WinUI and GTK hosts do not
yet consume it. The `apps/cli/` Rust binary exposes pipeline
profiles, catalogs, manifests, `dtk`, and DolphinTool without replacing the
upstream executables. The current pipeline stages the shared menu and Gecko
WebAssembly runtime as one Wii OS product distribution; native Windows uses
the Dolphin Core through its in-process ABI.

For user content, distinguish configuration from persistent user data. The
app may document a common logical tree (`roms`, `saves`, `savestates`, `bios`,
`brew`, `wad`, `assets`, `dump`, `sd-card`), but actual writable paths should
be per-user:

| Target | Config | User data and content |
|---|---|---|
| Windows | `%APPDATA%\\WiiOS` | `%LOCALAPPDATA%\\WiiOS` for saves/cache; user-selected content roots |
| Linux | `$XDG_CONFIG_HOME/wii-os` | `$XDG_DATA_HOME/wii-os` for saves/NAND; `$XDG_CACHE_HOME/wii-os` for cache; content roots selected by the user |
| Browser | Origin-local settings | IndexedDB/OPFS where supported, with explicit user file import/export; never claim arbitrary host filesystem access |

Do not put mutable saves, dumps, NAND, WADs, ROMs, keys or firmware under
`/usr` or inside a read-only app bundle. `userland/WII-RUNTIME-LAYOUT.md`
describes the current Linux staging tree; use it as a logical layout, not as a
recommendation to make `/usr/share` writable for normal users. Browser data is
origin-scoped and must be exportable/backed up by the user.

## Build and delivery contract

Each build run is isolated under a unique report directory. Steps record
command/log, exit status, skipped gate and artifact paths; the report captures
tool versions and source checkout revisions. `artifactManifest` records byte
size and SHA-256 for explicitly listed files, plus direct-file counts and bytes
for directories. Keep generated files out of upstream checkouts. A missing
dependency is `skipped` with its actual gate; never mark it passed or silently
install a toolchain. Pin source commits and submodules in the source revision
report. Build from clean Windows and Linux
CI runners, with a separate WASM build and browser qualification lane; Wii
homebrew remains a separate `powerpc-eabi` lane.

Minimum delivery matrix:

| Lane | Build proof | Runtime proof |
|---|---|---|
| Windows x64 | MSVC/CMake or Cargo output; D3D12-enabled host package | Launch app and record selected adapter/backend; compare fixed-reference captures |
| Linux x64 | Native GTK4/Wayland package and declared distro dependencies | Wayland and X11 where supported, theme/accessibility/focus and GPU adapter evidence |
| Web | Gecko `wasm-pack` output, static menu and emulator entry points, offline/self-hosted dependencies | HTTPS/localhost browser with WebGPU feature/adapter check, keyboard/gamepad and storage behavior |
| Wii PowerPC | devkitPPC/libogc homebrew `.dol`/`.elf` only | Console/emulator check of generated homebrew, distinct from host UI qualification |

Web release requirements: no mandatory CDN scripts/fonts, versioned static
assets, feature detection for WebGPU, clear unsupported-browser state, secure
context instructions, responsive fixed-aspect menu region, keyboard-visible
focus, reduced-motion option, audio consent/unlock handling, and capability
fallback for the non-emulator launcher UI. WebGPU being present in a source
dependency is not proof of browser rendering. Keep the WebGPU emulator and
menu shell separate until a real shared renderer and launch API are built.

Pixel parity is an acceptance test, not a design adjective. Record authorized
reference and candidate screenshots at identical region, firmware, state,
channel data, aspect ratio, resolution, scale/filter, color profile and timing.
Compare at least 640x480 and 854x480, annotate font/asset provenance, and
report per-screen visual and behavior differences. The current feature matrix
contains partial/no rows and there is no physical Wii screenshot fixture in
this checkout, so the target is unqualified.

## Source, asset, and distribution gates

- `Wii.JS` declares GPL-3.0. If the staged site redistributes its code, ship
  the required corresponding source and GPL notice, preserve upstream origin,
  and mark local changes. Its assets still need individual provenance; the
  repository-level license does not automatically establish ownership of
  console artwork, sounds, fonts or title icons.
- Gecko declares GPL-3.0; Dolphin's aggregate source is GPL-3.0-compatible
  with file-level SPDX notices and bundled license texts. A binary distribution
  needs the applicable source, notices and dependency license bundle.
- `decomp-toolkit` is MIT OR Apache-2.0. `wii-ipl` source is CC0-1.0, but
  its output build still requires a WAD from the operator; the license of the
  source reconstruction does not grant rights to that WAD or its assets.
- Keep every WAD, NAND, disc image, key, BIOS/firmware and proprietary asset
  operator-provided and out of repository, CI cache, logs, site output and
  release packages. Do not invoke downloader repositories as a build step.
- Before staging any asset, record origin, license/permission, author,
  allowed targets, region, content hash and whether redistribution is allowed.
  Unknown provenance blocks public packaging; user-local preview can remain
  opt-in without uploading the file.

## References used

- Local source evidence: `wii-os/pipeline.catalog.json`,
  `wii-os/dependencies.json`, `wii-os/formats.catalog.json`,
  `userland/WII-RUNTIME-LAYOUT.md`, and each checkout's own license/workflow.
- Current wgpu/winit platform and event-loop docs via Context7:
  [wgpu supported platforms](https://github.com/gfx-rs/wgpu/blob/trunk/README.md),
  [winit web event loop](https://docs.rs/winit/latest/winit/platform/web/trait.EventLoopExtWebSys.html).
- WinUI composition documentation via Microsoft Learn:
  [CreateSwapChainForComposition](https://learn.microsoft.com/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgifactory2-createswapchainforcomposition),
  [Direct3D 12 swap chains](https://learn.microsoft.com/windows/win32/direct3d12/swap-chains).
- GitHub connector repository search timed out during this audit; source
  identity and remotes were read from the local Git checkouts instead.
