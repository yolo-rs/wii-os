# Consolidated native source catalog

The Wii OS product gathers native C/C++ sources through a generated provenance inventory and the curated [C projects catalog](c-projects.catalog.json). Dolphin's C++ tree is under `src/dolphin`; Gecko's extracted crates live in the root `crates/gecko-*` workspace. Their distinct licenses, SDK assumptions and build rules remain recorded. The inventory indexes product-owned roots without creating a duplicate external source union.

Regenerate it with:

```powershell
./wii-os/pipeline.ps1 -Profile c-inventory
```

The report is written to `userland/build/reports/c-source-inventory/inventory.json`. It indexes C and C++ translation units and headers, SHA-256 hashes, source revisions, tracked or untracked state when available, source root, build adapter, and license-file paths plus hashes. `.c` translation units are identified as C; `.h` and `.hh` files are marked as language-ambiguous because they can be C, C++, or shared headers. Dolphin and Gecko use their in-product source snapshots and revision records in `src/dolphin/SOURCE-REVISION.txt` and `crates/gecko-core/SOURCE-REVISION.txt`.

## Source and build map

| Source root | Native role | Build route | Provenance constraints |
| --- | --- | --- | --- |
| `sources/wii-ipl` | Wii IPL/system software reconstruction; C and C++ | `wii-ipl` adapter | WIP; requires user-supplied WAD/config/build inputs. Root CC0 does not override nested third-party and SDK terms. No proprietary assets are included. |
| `sources/wii-examples` | Wii homebrew examples in C | `devkitpro-make` adapter | Requires devkitPPC, libogc and `DEVKITPPC/wii_rules`. No root license was found in the tracked tree; do not redistribute without resolving the applicable rights. |
| `sources/wiimms-iso-tools` | Wii disc/container tooling in C | `unix-make` adapter | GPL-2.0-or-later; generated setup and its Unix Make flow stay owned by this checkout. |
| `src/dolphin` | Wii/GameCube emulator, mostly C++ with some C | Qt-free CMake/FFI route | GPL-2.0-or-later; pinned product source snapshot with required notices. |
| `crates/gecko-*` | Wii/GameCube emulator, mostly Rust, with WebAssembly frontend | Wii OS root Cargo workspace | GPL-3.0 plus local asset/spec notices; pinned product source snapshot. |
| `wii-os/crates/menu-core` | Shared Wii menu model C ABI and C host example | `menu-core` native/WASM profiles | MIT; `include/wii_menu_core.h` is the public C contract backed by the Rust core. |

Other local product components are represented only when their source roots are explicitly listed in `source-roots.json`; this avoids sweeping unrelated directories or generated build trees from the machine.

The current shared C integration point is `crates/menu-core/include/wii_menu_core.h`, consumed by `crates/menu-core/examples/c/menu_host.c`. Dolphin remains the native emulator core behind the Rust-safe FFI; Gecko owns the Rust/WebAssembly browser path. The product selects each runtime through the same menu and launch catalog.

## Wii Menu assets

`sources/wii-ipl` contains decompilation source and references to assets stored in a user-supplied WAD; it does not contain the Wii Menu textures, fonts, animation files, banners, or boot video. The checked-in `wii-os/frontend/assets` carries the menu reproduction's Wii-style layout textures, cursor, Wii logo, Mii Channel preview and sound effects; the web UI binds channel tiles from `menu.catalog.json` instead of repeating that Mii preview on every slot.

For locally owned/extracted channel banner art, put files under `userland/data/wii/assets/channels/<title-id>/` and reference them with an `icon` such as `assets/user/channels/<title-id>/icon.png` in `userland/data/wii/menu.catalog.json`. The build prefers that local catalog over `menu/catalog.example.json`. For a boot clip, place `wii-boot.mp4` at `userland/data/wii/assets/boot/wii-boot.mp4`; the web build stages it as an optional local asset and plays it muted with a skip control. Without that file, the checked-in Wii logo animation and startup sound run. These local assets are ignored by Git and are not included in a release automatically.

The menu preview catalog is intentionally not a dump of all installed Wii channels. Add entries only for titles whose local content is configured; add their local icon paths to the catalog. The sample Mii preview is a bundled Wii.JS project asset, while all other Wii title artwork and boot footage must come from the user's own authorized files.
