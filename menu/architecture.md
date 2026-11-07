# Menu reconstruction architecture

This is the implementation map for the Wii OS menu slice in `recon.md`. It
describes the current hosts and target boundary; it does not claim one unified
pixel-perfect renderer or a Wii System Menu boot.

```text
channel manifest + user settings + input actions
                  |
        shared menu-core state/effects
          /            |             \
 browser adapter   native wgpu     GTK4 host
 DOM + WebGPU      winit + DX12     GTK widgets
          \            |             /
        typed launch request/result contract
                  |
          Dolphin / Gecko runtime
```

The shared catalog, transitions and launch vocabulary belong in
`crates/menu-core` and `crates/menu-runtime`. Hosts own presentation, storage
and device discovery. Emulator processes own WAD, NAND, IOS and title
execution. The menu must not parse or install WADs in its presentation layer.

## Current host map

| Host | Entry | Shared pieces | Current presentation | Known gap |
|---|---|---|---|---|
| Web | `frontend/index.html`, `frontend/assets/js/wii-os-adapter.js` | catalog and WASM-backed menu state | DOM/CSS shell with WebGPU channel artwork and DOM fallback | full WebGPU shell, focus model, persistent settings and browser qualification |
| Windows | `apps/menu-native` | menu model/runtime and catalog | winit + wgpu, DX12 selected in the recorded RTX 4070 run | WinUI shell, settings, broader device coverage and reference comparison |
| Linux | `apps/menu-native`, `apps/menu-gtk` | menu model/runtime and catalog | winit/wgpu scene and separate GTK4 host | shared canvas, persistent settings, controller focus and compositor qualification |

## Contracts and ownership

- `MenuState` is deterministic and serializable; platform handles stay out of
  the shared model.
- Hosts translate keyboard, pointer, browser gamepad, native controller and
  accessibility input into shared actions: focus, select, back, page, settings
  and launch.
- Catalog entries use stable IDs and explicit local content paths. User data
  resolution belongs to a platform storage adapter, never a hard-coded `/usr`
  write path.
- Launch requests carry catalog ID and validated content identity. Adapters
  return accepted, started, failed or cancelled status with a useful message.
  The UI does not infer success from process creation alone.
- The asset manifest records source, rights/provenance, hash, region, locale
  and intended use. Operator-provided console assets remain external.
- A host may fall back to DOM or native controls when WebGPU is unavailable,
  but diagnostics must expose the active renderer and fallback state.

## Delivery order

1. Establish shared visual tokens and a fixed logical canvas/scaling policy.
2. Complete S01/S02 channel focus, preview, back, launch status and empty/error
   states across the web host.
3. Complete S03 settings navigation and label Wii OS settings accurately;
   do not represent them as Nintendo System Menu settings.
4. Bring native and GTK hosts to shared action/state parity.
5. Capture each same state at a documented viewport and compare only against
   an authorized matching physical-console reference.

S04 is a placeholder, not a Wii Settings screen. S05-S07 still need complete
keyboard/controller and accessibility states.

## Outside the UI layer

System Menu IPL execution, WAD installation, ticket/TMD validation, IOS and
NAND behavior belong to emulator/runtime work. The supplied Budokai Tenkaichi 3
`main.dol` is game code and is not evidence about the System Menu.
