# Wii Menu integration boundary

The web distribution produced by `pipeline.ps1 -Profile wasm` has two distinct
entry points:

- `site/menu/` is the maintained Wii.JS browser reproduction. The build stages
  this source, the shared menu catalog, user-local assets and Rust/WASM
  navigation state together. Its WebGPU layer draws channel icon textures;
  shell controls and transitions still use HTML/CSS/DOM, with a DOM artwork
  fallback when WebGPU is unavailable.
- `site/emulator/` is Gecko's WebGPU/WebAssembly emulator. It accepts locally
  selected `.rvz` discs in bounded browser chunks and retains its `.dol`/`.bin`
  path. Disc data stays on the user's device; the `.wasm` is the emulator, not
  a recompiled game. Wii System Menu WAD/NAND boot is still unsupported in
  Gecko Web.
- `site/wii-files/` stages only the Wii WAD, IOS and Homebrew files referenced
  by the selected local menu catalog, and records hashes in `manifest.json`.
  The build includes the local PAL System Menu 4.3E WAD and IOS80 dependency
  when present. This makes the data available to a self-hosted site but does
  not install WADs, emulate NAND, or boot the real System Menu.
- The local Wii menu catalog includes the supplied Budokai Tenkaichi 3 RVZ.
  The browser build stages that RVZ under `site/wii-files/games/` and routes
  its channel to Gecko, which streams the RVZ into the WASM emulator. The RVZ
  remains runtime game data; this is not a PowerPC-to-WASM static recompilation.

The Windows and Linux route for an actual System Menu title is Dolphin's
existing WAD boot path, with an operator-provided, authorized WAD and NAND. The
Wii IPL decompilation is a separate, incomplete source reconstruction that
requires an operator-provided WAD and produces `main.dol`; it is not a web
renderer or a WAD installer. Do not label the Wii.JS prototype as pixel-perfect
or the Gecko Web build as a System Menu boot until each claim is qualified
against a supplied reference and a real browser/runtime.

Wii.JS binds up to twelve tiles per page from the shared catalog. For a local
installation, set `userland/data/wii/menu.catalog.json`; the web
build and control-plane server prefer it over the example catalog. A Wii WAD
entry can point `contentPath` at a local relative file such as
`userland/data/wads/<title-id>.wad`; the server can launch a configured local
`.wad` through Dolphin or boot a title already installed in the configured
Dolphin NAND profile by its 16-digit title ID. Tile icons may point at
`assets/user/channels/<title-id>/icon.png`, staged from the local asset tree.
An optional `assets/user/boot/wii-boot.mp4` is played at startup; no Wii boot
video or WAD-derived channel banner set is bundled by this coordinator. The
browser keeps the bundled Wii logo startup animation and can play a local boot
video before it; Wii.JS also includes local menu artwork and a Mii Channel
preview. These assets do not establish pixel-perfect parity with a real menu.

## Integration contract

The coordinator builds Dolphin from `src/dolphin` and Gecko from the root
`crates/gecko-*` workspace, then stages the static menu, selected local title data, and Gecko site into one
product distribution. Gecko Web has a generic Wii game disc boot path. Booting the actual
Wii System Menu still needs WAD ticket/TMD/content installation, IOS and NAND
semantics, and persistent browser storage.
Native Windows can use Dolphin's D3D12 backend; browser WebGPU remains the
browser-only graphics API. Linux desktop integration remains a separate
Wayland/GTK layer.
