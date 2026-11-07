# `wii` command line

Build the portable Windows/Linux coordinator from the workspace root:

```powershell
./wii-os/pipeline.ps1 -Profile cli
./userland/build/targets/wii-cli-bootstrap/release/wii.exe doctor
./userland/build/targets/wii-cli-bootstrap/release/wii.exe catalog menu
./userland/build/targets/wii-cli-bootstrap/release/wii.exe menu-model
./userland/build/targets/wii-cli-bootstrap/release/wii.exe build --profile web
./userland/build/targets/wii-cli-bootstrap/release/wii.exe build --profile homebrew
./userland/build/targets/wii-cli-bootstrap/release/wii.exe build --profile server
./userland/build/targets/wii-cli-bootstrap/release/wii.exe service
./userland/build/targets/wii-cli-bootstrap/release/wii.exe dtk -- disc info ./content/game.iso
./userland/build/targets/wii-cli-bootstrap/release/wii.exe dolphin-tool -- header ./content/game.iso
./userland/build/targets/wii-cli-bootstrap/release/wii.exe run --exe ./path/to/emulator -- --help
```

PowerShell 7 (`pwsh`) is needed for coordinator-backed commands. Set
`WII_OS_WORKSPACE` or pass `--workspace` when invoking the executable outside
the workspace. The CLI resolves catalogs and manifests relative to that root,
passes external process arguments without shell interpolation, and returns the
child process exit status. It never downloads games, WADs, NANDs, keys or BIOS.

`dolphin-tool` and `dtk` are passthrough adapters: upstream remains the owner
of its parser and supported command surface. Build Dolphin's no-Qt targets
with `wii build --profile build --target host`; the binary may be in that
run's `dolphin-build/` output tree, so pass `--exe` if it is not on `PATH`.
Use `run --exe <DolphinNoGUI> -- -e <operator-supplied-content>` to launch a
local Dolphin frontend explicitly. WAD recognition/boot is an emulator path,
not a DolphinTool operation or a promise that every WAD/NAND boots.

## Current command surface

- `doctor`: source revision, toolchain and gate report.
- `catalog repositories|dependencies|formats|menu`: machine-readable JSON, including the shared Wii app catalog.
- `build --profile build|all|web|cli|homebrew|server --target ...`: shared pipeline.
- `inspect` and `validate`: explicit input manifest profiles.
- `dtk` and `dolphin-tool`: argument-preserving upstream command adapters.
- `run`: explicitly launch a configured local executable with arguments.
- `menu-model --catalog <JSON>`: validate and load the shared catalog into the Rust menu state core.
- `service`: explicitly start the HTTP/WebSocket hub. Set `WII_OS_WEB_ROOT` to a built `site/` directory to host Wii.JS at `/menu/` beside the API. Configure `WII_OS_DOLPHIN_EXE` and the user-owned `WII_OS_SYSTEM_MENU_WAD`, `WII_OS_MII_CHANNEL_WAD`, and `WII_OS_SHOP_CHANNEL_WAD` variables to enable file-based title launches. The default Dolphin profile is the isolated European profile at `userland/data/dolphin-wii-eu`; override it with `WII_OS_DOLPHIN_USER_DIR`. Persistent rooms live in `userland/data/wii-os.sqlite3` by default; set `WII_OS_DATABASE` to move the database. The former `WII_OS_ROOM_STORE` JSON file is imported once if the database is empty and is retained. Remote binds require `WII_OS_API_TOKEN` (at least 32 bytes); clients may send it as a Bearer token. Browser clients use a trusted reverse proxy that authenticates users and issues an `HttpOnly; Secure; SameSite=Strict` `wii_os_token` cookie. Keep the default loopback bind for local use and terminate TLS at a reverse proxy for remote access.

The command surface deliberately delegates rather than reimplementing every
repository. Service startup and title launches are explicit. The shared native
runtime passes user-provided WAD files and homebrew DOLs to Dolphin with
`--exec`; Dolphin's file boot path recognizes WADs. A WAD launch still depends
on a compatible user-provided IOS/NAND setup, and the launcher does not install
WADs persistently or provide system files. Its lobby and WebSocket are not a
game-state transport or Dolphin NetPlay replacement.
