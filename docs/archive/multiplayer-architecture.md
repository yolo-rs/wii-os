# Wii OS multiplayer and platform integration (2026)

## Decision

Keep game synchronization and the social/control plane as different protocols.
The product contains the Dolphin and Gecko emulator engines and a Go WWFC server. Wii OS now
has a first Rust control-plane prototype using Tokio + Axum for HTTP and
WebSocket routes. It currently covers health, in-memory room creation/listing
and event broadcast; presence, invites, chat and session permissions still need
implementation. WebSocket messages are not frame input and do not replace
Dolphin NetPlay.

| Use | Selected technology | Why it fits this workspace | Status |
| --- | --- | --- | --- |
| Dolphin native netplay | Existing Dolphin Core NetPlay / ENet / SFML Network | Already part of the emulator stack; preserves sync, buffering and compatibility | Qt-free C ABI adapter and Rust session wrapper are integrated; direct LAN session still needs end-to-end qualification |
| Lobby/API/WebSocket | Rust Tokio + Axum | Rust is already central to Gecko, menu-core and CLI; one HTTP+WS service can serve browser/native clients | Prototype builds and exposes HTTP/WS routes |
| Wii/DS online services | Existing Go `wfc-server` (WWFC) | Implements legacy Nintendo console protocols and already has its own Go/PostgreSQL/gVisor dependency graph | Keep separate and opt-in |
| Browser peer game data | WebRTC DataChannel, with Pion as a signaling/service candidate | Browser-native transport option for a future Gecko/browser runtime | Defer; no current browser Dolphin NetPlay endpoint |
| Valve GameNetworkingSockets | Not selected for core transport | Capable cross-platform game transport, but duplicates Dolphin NetPlay and adds a separate native dependency; it does not solve browser transport | Optional future evaluation |
| WebSocket-only gameplay sync | Rejected for deterministic Dolphin netplay | Useful for lobby/control messages; not a drop-in replacement for emulator NetPlay | — |

The existing repo uses SDL3, XInput, DirectInput and platform input backends.
Preserve those paths. Use Dolphin's SDL3 backend as the common native gamepad
path and keep XInput/DirectInput as Windows backends. Add GameInput only as an
optional Windows 11/Game Development Kit adapter after qualification. In the
browser, use the Gamepad API for keyboard-adjacent gamepad navigation and
controls; browser permission/focus behavior means devices and advanced haptics
cannot be promised uniformly. Wii Remote and Wii U-specific features remain
bound to Dolphin's supported device backends, not a generic controller claim.

Discord is three integrations, not one SDK: Embedded App SDK for the browser
Activity; Discord API bot using a bot token held server-side; and optional
Discord Social SDK C++ for native social features. Keep all behind feature
flags and application credentials. Do not make a legacy Discord Game SDK a
build requirement.

## Ports and safe startup

The prototype serves HTTP and WebSocket on one configurable listener at
`127.0.0.1:8780` by default. It is disabled until explicitly started; public
listening requires explicit operator configuration. Dolphin NetPlay
and WWFC retain their own port settings and lifecycle. The service must not
silently install firewall rules, open public listeners, or start a database.

## Current repository evidence

- `Cargo.toml` is the sole Rust workspace and includes Gecko's extracted
  native and WebAssembly crates alongside Wii OS and Dolphin FFI crates.
- Dolphin and Gecko are maintained from their in-product source trees. Gecko's
  Rust WebAssembly target and Dolphin's Qt-free C ABI are separate runtime paths;
  Dolphin includes SDL3, XInput and DirectInput backends.
- `sources/wfc-server/go.mod` pins a Go 1.25 module and includes PostgreSQL and
  gVisor dependencies. It is a legacy Wii/DS online service, not a lobby for
  Dolphin sessions.
- `wii-os/pipeline.catalog.json` already builds `wfc-server` independently;
  `wii-os/pipeline.ps1 -Profile server` builds the separate Wii OS hub.

## Candidate comparison

- **Axum/Tokio** is selected for the new lobby/control plane because Rust is
  already present in the CLI, shared model and Gecko. Axum supplies HTTP routes
  and WebSocket upgrade on the same service. The current prototype pins Axum
  0.8 in the root `Cargo.lock`.
- **Go + Pion** is a credible browser WebRTC/signaling option and would fit the
  existing Go services, but coupling new lobby APIs into WWFC would mix separate
  protocol/service responsibilities. Reconsider if the team chooses one Go
  operations boundary for all network services.
- **ENet** is the correct reuse choice for Dolphin NetPlay, not a new generic
  Wii OS lobby dependency.
- **Valve GameNetworkingSockets** is a strong native option for a new game
  protocol with reliability, encryption and NAT traversal, but adds a native
  stack and has no browser endpoint. Do not layer it over Dolphin NetPlay.
- **WebSocket** is the interoperable control-plane link; **WebRTC DataChannel**
  is a possible later browser peer path. They serve different use cases.

## Discord, Windows and controllers

- Discord Embedded App SDK: browser Activity frontend.
- Discord API bot: separate bot identity and server-only secrets.
- Discord Social SDK: optional native C++ integration; it is not the web
  Activity SDK and does not supply Wii OS UI.
- Windows GameInput: optional native adapter for newer Windows/Xbox APIs.
  Retain Dolphin's XInput/DirectInput/SDL paths to avoid regressions. Xbox GDK
  distribution/console targets require appropriate Microsoft partner access;
  Windows SDK/GameInput PC support does not grant console publishing access.
- Native gamepad profiles remain Dolphin-managed. Shared Wii OS navigation
  actions may map to standardized logical controls without overwriting game
  profiles. Web controls use `navigator.getGamepads()` and keyboard/mouse.

## Catalog and distribution

The catalog can preload metadata, supported formats, hashes computed from
user-owned files, presets, and local paths. It must not ship Nintendo ROMs,
WADs, BIOS, NAND or copyrighted artwork. Users select local files or provide
their own content. The same catalog can point to local `roms`, `wad`, `bios`,
`saves`, `savestates`, `brew`, `assets` and dump directories under Wii OS's
userland without copying files into the application package.

## Implementation sequence

1. Extend the `wii-os-server` Rust prototype with auth, catalog, persistent
   room state, presence and invite flows; keep all listeners opt-in.
2. Define a versioned wire schema in `menu-core` and generate/use it from the
   native CLI, web client and Discord Activity.
3. Add CLI `service status`/`stop` management and durable service supervision.
4. Add an explicit Dolphin integration boundary that calls the existing
   NetPlay interface and reports status; do not reimplement its protocol.
5. Add the WebRTC path only if browser Gecko multiplayer is implemented.
6. Add Discord integrations and GameInput behind independent Cargo/build
   features, with credentials and proprietary SDKs supplied by the operator.
7. Qualify controllers per OS/device, WebGPU browsers, NAT scenarios, reconnect,
   identity, room permissions, and compatibility before claiming full support.

The HTTP/WebSocket hub is implemented in `apps/server/` and can be built with
`wii build --profile server`, then started explicitly with `wii service`. It
serves the embedded installable Wii OS Web interface at `/`, reports
`/healthz`, lists user-configured titles at `/v1/apps`, starts them through Dolphin at
`POST /v1/apps/{id}/launch`, exposes `GET/POST /v1/rooms`, and emits lobby
events on `/v1/ws`. It loads the shared versioned menu catalog; setting
`WII_OS_WEB_ROOT` to a built `site/` directory optionally serves Gecko WebGPU
at `/emulator/` and menu assets at `/menu/` on the same origin; the embedded
Wii OS home remains at `/`. The interface uses the same API, SQLite database,
WebSocket and MCP service as the native product. Configure
`WII_OS_DOLPHIN_EXE`, `WII_OS_SYSTEM_MENU_WAD`,
`WII_OS_MII_CHANNEL_WAD`, and `WII_OS_SHOP_CHANNEL_WAD`. All titles use the
same isolated Dolphin user directory by default. Dolphin's `--user` and
`--exec` options pass each configured WAD through Dolphin's WAD file boot
path. The operator still needs a compatible, user-provided IOS/NAND setup in
that profile; the hub does not import IOS or install WADs permanently. Mii
data therefore shares the profile's NAND state when that setup is valid.

The lobby tracks room membership over WebSocket. Every `/v1/*` API route,
WebSocket and MCP request accepts a Bearer token or `wii_os_token` cookie when
`WII_OS_API_TOKEN` is configured. The web login exchanges that secret for an
eight-hour `HttpOnly; SameSite=Strict` cookie. Non-loopback binds require a
URL-safe secret of at least 32 characters and `WII_OS_COOKIE_SECURE=1`; terminate
TLS at a reverse proxy and restrict access to its upstream port. Room
definitions persist in `userland/data/wii-os.sqlite3` by default,
configurable with `WII_OS_DATABASE`. SQLite is bundled into the service, uses
WAL with full synchronous commits, and has a versioned migration path. An
existing JSON room store is imported once and retained; use `WII_OS_ROOM_STORE`
to point at a non-default legacy JSON file. Live player counts reset on restart.
The same database persists the Dolphin video backend through `GET /v1/settings`
and `PUT /v1/settings` (`videoBackend`: `D3D`, `D3D12`, `Vulkan`, `OpenGL`, or
`Software`). Launches use this setting unless
`WII_OS_DOLPHIN_VIDEO_BACKEND` is set. The settings write route requires the
same API-token protection as room creation when authentication is configured.
The lobby has no participant identity,
Discord integration or Dolphin NetPlay session API, and rooms do not carry
gameplay packets. A Shop Channel
WAD still depends on compatible IOS/region state; the hub does not restore
Nintendo's discontinued official Shop service. Treat it as a control-plane
prototype until presence, identity, NetPlay sessions, and end-to-end deployment
qualification are complete. Persistent room definitions are durable now.
