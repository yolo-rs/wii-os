# Menu build log

Statuses reflect source inspection. No clean-room screen code or screenshot
comparison was performed in this recon/build pass.

| Screen | Status | Evidence | Remaining work |
|---|---|---|---|
| S01 startup and grid | Partial | `frontend/index.html`, `views/menu.html`, `assets/js/views.js` | reference capture, full accessibility states and host parity |
| S02 channel preview | Partial | `assets/js/wii-os-adapter.js`, `webgpu-channel-layer.js` | launch-result UI, end-to-end result states, exact reference timing/scale |
| S03 settings overview | Partial | `views/settings-main.html` | working Info/Credits, correctly named Wii OS settings and keyboard focus |
| S04 license placeholder | Incomplete | `views/licenses-temp.html` | remove from the Wii Settings path or implement a clearly named app license screen |
| S05 resolution notice | Partial | `assets/js/views.js`, `universal.css` | keyboard dismissal, persisted preference and accessible announcement |
| S06 empty channel | Partial | `views/menu.html`, channel adapter | accessible name and consistent keyboard/controller focus |
| S07 mail action | Incomplete | `views/menu.html` | implement an app action or expose a disabled state |

## Qualification record

- Reference screenshots: none available in the checkout.
- Candidate screenshots: not captured in this pass.
- Browser/WebGPU: source contains adapter and DOM fallback paths; no live
  browser qualification was performed in this pass.
- Native Windows/Linux: see `windows.md` and `linux.md`; those observations do
  not qualify the web screens.
- The shared token stylesheet is linked from the web entry point and supplies
  a documented visual baseline for subsequent screen work.
- Commit: unavailable; `C:/src/wii/wii-os` is not inside a Git worktree in the
  current filesystem view.
