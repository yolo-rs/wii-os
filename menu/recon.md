# Wii menu replication reconnaissance

## Scope used

- Product slice: Wii Menu 4.3-style shell, channel grid, channel preview, and
  settings shell across desktop Windows, browser, and Linux desktop.
- Audience: Wii OS users running locally supplied, authorized content.
- Current checked-in frontend: `wii-os/frontend` (derived from upstream `andrewplus/Wii.JS`,
  GPL-3.0). The actual Wii Menu decompilation is a separate source project
  (`sources/wii-ipl`) that requires an operator-supplied WAD and is not a UI
  reference implementation. No user-provided capture of a physical Wii is
  checked into this workspace, so pixel-level parity cannot yet be scored.

This is a reconstruction map of the checked-in browser prototype, not a claim
that the prototype is equivalent to the actual Nintendo System Menu.

## Source and evidence map

| Source | Evidence | Limits |
|---|---|---|
| `wii-os/frontend/index.html`, `views/*.html` | Screens and visible controls | App shell with local startup sequence and menu/settings views |
| `wii-os/frontend/assets/js/{views,main}.js` and `wii-os/frontend/assets/js/wii-os-adapter.js` | View changes, transitions, audio, shared state, catalog, standard Gamepad API input, keyboard input, and launch requests | Browser still depends on DOM for shell and transitions; WebGPU currently covers channel artwork only |
| `wii-os/frontend/assets/css/{menu,settings,universal}.css` | Layout, transitions, responsive composition | Pixel parity against physical Wii capture is not qualified |
| `wii-os/apps/menu-native/src/main.rs`, `apps/menu-gtk`, and `crates/menu-core` | Native winit/wgpu and GTK4 hosts, shared channel state, keyboard/mouse navigation and configured Dolphin WAD/DOL launch | DX12 initialized on one Windows RTX 4070; Linux host build observed but desktop/controller/settings parity and physical-reference diff are unqualified |
| `sources/wii-ipl/README.md` | Actual System Menu 4.3 decompilation scope and required WAD | Incomplete reconstruction; contains no proprietary assets/executable |
| Physical console reference capture | Not available in this checkout | Required to measure pixel similarity for a given region/version/state |

## Screens and states observed

| ID | Screen/state | Entry | Components | Evidence / gaps |
|---|---|---|---|---|
| S01 | Startup fade then channel grid | Automatic page load | Black fade, channels, footer, date, mail, settings, warning overlay | JS switches to `menu`; audio autoplay is browser-gated |
| S02 | Channel preview | Select an occupied grid cell | Zoomed menu, image-backed preview, Wii Menu and Start controls | Start emits a launch request; configured local server/emulator and catalog entry are required; result feedback is not a full console launch flow |
| S03 | Settings overview | Click lower-left Wii button | Settings header, Info/Credits, Wii Settings, Back | Info/Credits is inert; Wii Settings opens a placeholder license view, not system settings |
| S04 | Temporary license view | Settings overview → Wii Settings | Third-party waitForImages license text and link | Not actual Wii Settings; only one dependency is documented there |
| S05 | Compatibility warning dismissed | Click warning | Warning overlay | No keyboard dismissal or remembered preference |
| S06 | Blank channel hover | Hover a blank grid cell | Empty channel tile | No action or accessible name |
| S07 | Mail control | Click/hover lower-right button | Mail icon | No click behavior wired |

## Flows observed

- F01 Boot shell: load `index.html` → fade → menu. First-use music/splash is
  subject to autoplay policy and currently has visible audio controls.
- F02 Preview a channel: click occupied tile → zoom preview → Wii Menu returns
  to grid. Web Start posts a launch request to the local server; the native
  renderer launches a validated local WAD/DOL through configured Dolphin
  executable. Neither path reproduces the console's complete channel transition.
- F03 Open settings: grid → settings → temporary license view → Back to grid.
- F04 Dismiss resolution notice: click overlay → notice hidden for this page
  session only.

## Replication requirements for the native and web shell

Use one screen/state model, channel manifest, input action vocabulary, and
assets/provenance manifest shared by native Windows, WebGPU/Web and Linux.
Implement presentation backends independently: browser WebGPU for web; native
wgpu with D3D12 selection on Windows; Linux Wayland/GTK4 host with a shared
render surface. Native platform chrome and Linux desktop integration must not
change the 4:3/16:9 menu composition. Add deterministic clock/audio settings,
controller and keyboard focus, resize states, empty/loading/error channel
states, settings navigation, and launch-result feedback.

Pixel-perfect acceptance requires the same authorized region/firmware state,
channel manifest, aspect ratio, scaling mode, output resolution, and capture
pipeline. Capture a real reference and candidate at 4:3 640x480 and 16:9
854x480 first, then run the repository visual diff. A successful diff score
must be reported per screen/state; until then the target remains unqualified.

## Reverse-engineering evidence ledger

The supplied Dragon Ball Z Budokai Tenkaichi 3 PAL RVZ is game content, not a
System Menu reference image or System Menu executable. REA identified it as
Wii RVZ, game ID `RDSPAF`, with a 4,699,979,776-byte logical disc image and
SHA-256 `d3910a98ba3c068d721e8671ceb25418d55321af5f27f5b614a1837a5ac2714f`.
The matching extraction record is
`C:/src/wii/wii-os/userland/build/extractions/budokai-tenkaichi-3-eur-20261030T005541Z/README.md`.
It records successful Redump verification, extracted game/update partitions,
and Ghidra PowerPC pseudocode for the game's `main.dol`. That pseudocode is
not original source and gives no evidence about Wii Menu pixels or behavior.

Frontend observations in this map come from checked-in views, CSS and adapter
code, not a physical-console capture. Standard-mapped browser gamepad support
is present, but Wii Remote and per-device parity are not qualified. More
provenance and limitations are recorded in `evidence/rvz-and-menu.md`.

## Source and asset handling

`Wii.JS` has a repository GPL-3.0 license. The checked-in code may be staged
under that license with corresponding notices/source, while preserving its
upstream origin and clearly identifying modifications. Its image/audio files
do not have an asset-level provenance inventory in the checkout; do not assume
the repository license proves rights to redistribute Wii-branded artwork,
sounds, or title imagery. The current entry point does not load Google Fonts
or jQuery from a CDN, though a leftover temporary license view links to an
external project page. The next clean-room
native/web implementation should use independently sourced assets or
operator-provided assets with recorded rights and should not copy this
prototype's implementation or Nintendo UI assets.

## Size and hard parts

Estimated size: XL. Main risks are sourcing authorized pixel references and
assets, matching Wii timing/scaling/audio/input behavior, unifying channel
launch semantics with the emulator, and delivering native Windows and Linux
hosts without coupling the web UI to platform APIs. The actual System Menu
boot pipeline (WAD/TMD/ticket/content, IOS and NAND) is a separate emulator
feature and is not implemented by this UI reconstruction.
