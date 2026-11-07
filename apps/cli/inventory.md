# Unified `wii` CLI: source and adapter inventory

This inventory maps the 17 source checkouts plus the shared `menu-core` crate
to safe CLI surfaces. It is based on local manifests, READMEs, source entry
points, and the coordinator catalog. A unified CLI should invoke these tools
through explicit adapters; it should not copy their implementation into a
monolith or infer support from a file extension alone.

## User-facing command groups

| `wii` group | Backing project | Initial operation | Gate / policy |
|---|---|---|---|
| `doctor`, `repos`, `formats` | Wii OS catalog, dependency inventory and `menu-core` | Show revisions, tool availability, formats and platform gates | Read-only; report missing tools as `skipped` |
| `build`, `pipeline` | `wii-os/pipeline.ps1` + catalog adapters | Run selected profile/target and summarize its JSON report | Do not auto-install tools or initialize submodules; preserve per-repo logs |
| `disc info`, `disc verify`, `disc extract` | `dtk disc ...` | Inspect, verify and extract supported ISO/WIA/RVZ/WBFS/CISO/NFS/GCZ/TGC inputs | Require an explicit input path; write extraction only to explicit output |
| `disc convert`, `disc header` | `dolphin-tool convert|verify|header|extract` | Container conversion and metadata/verification | Explicit source and output; only supported container conversions; never emit EXE/ELF/WASM from game data |
| `run gecko` | `gecko` / `tinyapp` / `debugger` | Launch a supported DOL, disc or IPL with user-selected system files/NAND | Resolve binary from a configured tool path; validate input and log exact argv |
| `run dolphin` | `dolphin-emu` or `dolphin-emu-nogui` | Launch ordinary desktop UI or explicit batch/headless execution | WAD and NAND remain user inputs; headless runs must specify `--exec` or `--nand_title` and have an explicit stop/timeout policy |
| `wad inspect`, `wad extract`, `wad pack` | `dtk` first; optional Go `wadlib` library adapter | Read metadata/extract through supported `dtk`; future pack only with separately reviewed title-building recipe | `wadlib` is a Go library, not a CLI; no decryption/install behavior is implied; never expose downloader functionality |
| `homebrew build` | devkitPPC/libogc + `wii-examples` Makefiles | Build selected example to DOL/ELF | Require configured devkitPPC; no WAD creation by default; output stays in build report directory |
| `ipl build` | `wii-ipl` | Reproduce a supported region from the operator-supplied WAD into `main.dol` | Opt-in; region and input required; source decompilation is incomplete; preserve WAD provenance and do not package it |
| `archive index` | `sources/wii` | Index filenames/metadata for local archive review | Metadata-only by default; no extraction, download or redistribution |
| `web build`, `web serve` | Gecko `wasm-pack` output + Wii.JS static tree | Assemble static menu/emulator entry points and optionally serve locally | WebGPU runtime needs secure context; static UI is a reproduction, not the System Menu binary |
| `service build`, `service run` | `wfc-server`, `room-server` | Compile service tools; run only when operator supplies service configuration | No implicit network listener, database migration, credential lookup, or service startup |
| `desktop integrate` | `komorebi`, `yasb`, `smithay`, `wayland-rs` | Optional host/window-system integration diagnostics | Platform-specific and non-core; no automatic window-manager config mutation |
| `wiiu inspect` | `WiiUDownloader` source build (future) | Report project/tool availability only | GUI client, generated `db.go`, Go/toolchain and license-specific gates; no download operation in the default `wii` CLI |

## Repository affordance details

| Checkout | CLI/API surface and source evidence | License / role |
|---|---|---|
| `decomp-toolkit` | `dtk` (`sources/decomp-toolkit/src/main.rs`); README documents `dtk disc info`, `disc extract`, `vfs`, archive and format commands | MIT OR Apache-2.0; primary inspection frontend |
| `dolphin` | `DolphinTool` in `Source/Core/DolphinTool/ToolMain.cpp` exposes `convert`, `verify`, `header`, `extract`; `DolphinNoGUI/MainNoGUI.cpp` is the non-Qt runtime; CMake options include `ENABLE_CLI_TOOL`, `ENABLE_NOGUI`, `ENABLE_HEADLESS`, and `ENABLE_QT` | GPL-2.0-or-later; use the upstream headless target and notices, do not copy sources into the Rust CLI |
| `gecko` | Native binaries `gecko`, `tinyapp`, `debugger`, `multitool`; README documents `tinyapp --dvd <image>`, optional `--ipl`, `--dsp`, `--coef`, `--skip-ipl`, and Wii NAND via `GECKO_FS_ROOT` | GPL-3.0; extracted emulator/runtime and dev tools are members of the root Cargo workspace at `crates/gecko-*` |
| `gecko-web` | `wasm-pack build crates/gecko-web --target web --out-dir pkg --out-name gecko_web --release` | Same Gecko license; WebAssembly browser runtime is a root workspace member |
| `wadlib` | Go package/API for parsing, extracting and modifying WAD data; no standalone `main` package in the checkout | MIT; library adapter only, avoid duplicating `dtk` unless a consumer needs the Go API |
| `wii-examples` | Recursive Makefiles use devkitPPC/libogc to compile example DOLs; top-level `make examples` builds a broad set | Review per-file/template license and output provenance; optional PowerPC target |
| `wii-ipl` | Ninja workflow builds a selected supported region from an operator WAD and emits `main.dol` | CC0 source, but WAD/assets rights are independent; incomplete decompilation |
| `wiimms-iso-tools` | Unix CLI programs `wit`, `wwt`, `wdf`; `project/Makefile` and `project/dclib/dclib-cli.c` | GPL; optional Unix adapter, generated setup and license review before redistribution |
| `wii` | Local archive tree with no supported runtime CLI contract | Metadata indexing only until each item's license/provenance is reviewed |
| `Wii.JS` | Static HTML/CSS/JS (`index.html`, `assets/js/{main,views}.js`), no command-line app | GPL-3.0 repository license; assets need individual provenance; stage with matching source/notices for redistribution |
| `Wii-Rom-Downloader` | Python package/CLI source in `wii_roms_tool/src/main.py`; wheel packaging is separate from using it | Exclude downloads from `wii` default commands; user-authorized local input only, no automatic retrieval |
| `WiiUDownloader` | Desktop Go application entry point `cmd/WiiUDownloader`; README says GUI/no command-line flags; current package build is gated on generated `db.go` | GPLv3; do not present as Wii compiler, emulator, or default downloader command |
| `wfc-server` | Go service binaries/packages (root and helper `main.go` entry points) | Network service; `service run` must be explicit and isolated |
| `room-server` | Containerized Python/Postgres/OIDC/S3 service configuration | Separate service deployment; no source CLI should mutate databases or start listeners silently |
| `komorebi` | `komorebic` command tree and Windows window manager | Optional Windows shell integration; separate license and safety confirmation before changing OS state |
| `yasb` | Python tray app, `src/cli.py` and packaged app command | Optional Windows UI, not a Wii runtime dependency |
| `smithay` | Rust compositor library; `anvil`/`smallvil` are examples | Linux/Wayland only; do not treat compositor examples as menu application |
| `wayland-rs` | Rust client/server/protocol libraries and workspace tools | Linux integration dependency, not a Wii content CLI |

## Dolphin non-Qt build recommendation

Use the checked-in upstream targets rather than copying its “non-Qt” code into
`wii-os`: `dolphin-tool` is an independent command-line utility, and
`dolphin-emu-nogui` is the frontend without the Qt UI. Build the Dolphin tree
with CMake, keeping `ENABLE_CLI_TOOL=ON` and `ENABLE_NOGUI=ON`; set
`ENABLE_QT=OFF` (and `ENABLE_HEADLESS=ON` only for a headless worker build).
The exact build additionally depends on the Dolphin CMake graph, generated
revision metadata and initialized source submodules. The coordinator must
report that gate and retain Dolphin's source and GPL notices. `dolphin-tool`
handles image inspection/conversion; `dolphin-emu-nogui` handles emulation.
They are separate executables and should remain separate adapters.

## Invocation contract for the new CLI

The CLI should first expose the wrappers that are deterministic and local:

```text
wii doctor --json
wii repos list
wii formats list
wii pipeline audit|build [--target windows-x64|linux-x64|web-wasm32-webgpu|wii-powerpc]
wii disc info <path>
wii disc verify <path>
wii disc extract <path> --out <directory>
wii disc convert <path> --format rvz|iso|gcz|wia --out <path>
wii run gecko|dolphin <path> [--batch] [--nand <directory>]
wii homebrew build <example-directory>
```

Resolve tools from explicit config/environment or `PATH`; never search random
directories for an emulator. Pass argument arrays directly (no shell string
interpolation), preserve child exit codes, capture stdout/stderr and record
tool version, input/output hashes, target, revision, elapsed time and skips.
Destructive outputs require an explicit `--out`; default inspect/doctor/list
commands stay read-only. Don't add flags that accept or seek Nintendo keys,
title databases or remote ROM download sources. Keep credentials out of
reports and redact secret-looking environment values.
