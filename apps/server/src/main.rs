use std::{collections::BTreeMap, env, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::Request,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};
use tower_http::services::ServeDir;
use wii_menu_core::{MenuCatalog, WiiSysconf};
use wii_menu_runtime::{
    NetPlayHostRequest, NetPlayJoinRequest, NetPlayManager, PackageRequest, copy_nand_save_to_sd,
    delete_nand_save_entry, inspect_nand_summary, launch_catalog_channel_with_backend,
    list_built_packages, load_workspace_catalog, normalize_video_backend, package_game,
    prewarm_first_wii_disc, resolve_dolphin_executable, resolve_workspace_file,
    resolve_workspace_root,
};

mod db;
use db::{NewBoardMessage, RoomDatabase};
pub mod embedded_assets;
mod mcp;

#[derive(Clone)]
pub(crate) struct AppState {
    events: broadcast::Sender<ServerEvent>,
    rooms: Arc<RwLock<BTreeMap<String, Room>>>,
    database: RoomDatabase,
    workspace: std::path::PathBuf,
    catalog_path: std::path::PathBuf,
    catalog: Arc<RwLock<MenuCatalog>>,
    netplay: Arc<NetPlayManager>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Room {
    pub id: String,
    #[serde(alias = "title_id")]
    pub title_id: Option<String>,
    #[serde(skip_deserializing, default)]
    pub players: u32,
    #[serde(alias = "max_players")]
    pub max_players: u32,
    pub transport: String,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub host_nickname: Option<String>,
    #[serde(default)]
    pub host_address: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub pad_buffer: Option<u16>,
    #[serde(default)]
    pub game_digest: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub invite_code: Option<String>,
    #[serde(default)]
    pub invite_url: Option<String>,
}

pub(crate) fn generate_room_invite_code(room_id: &str) -> String {
    if room_id.len() <= 8 && room_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return room_id.to_uppercase();
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(room_id, &mut hasher);
    let h = std::hash::Hasher::finish(&hasher);
    format!("WII-{:04X}", (h & 0xFFFF) as u16)
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
struct ServerEvent {
    room: String,
    event: String,
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateRoom {
    pub(crate) id: String,
    #[serde(alias = "title_id")]
    pub(crate) title_id: Option<String>,
    #[serde(alias = "max_players")]
    pub(crate) max_players: Option<u32>,
    pub(crate) transport: Option<String>,
    pub(crate) channel_id: Option<String>,
    pub(crate) host_nickname: Option<String>,
    pub(crate) host_address: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) pad_buffer: Option<u16>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    JoinRoom {
        room: String,
    },
    LeaveRoom {
        room: String,
    },
    RoomChat {
        room: String,
        sender: String,
        message: String,
    },
    PadAssign {
        room: String,
        nickname: String,
        wiimote_port: u8,
        ready: Option<bool>,
    },
    WebrtcSignal {
        room: String,
        sender: String,
        signal: serde_json::Value,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    id: String,
    channel_id: String,
    title: String,
    title_id: Option<String>,
    content_kind: wii_menu_core::ContentKind,
    icon: Option<String>,
    blocks: u32,
    publisher: String,
    players: u8,
    content_ready: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) video_backend: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelMoveInput {
    pub(crate) channel_id: String,
    pub(crate) preferred_page: usize,
    pub(crate) preferred_slot: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoomChatInput {
    pub(crate) sender: String,
    pub(crate) message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetPlayPadInput {
    pub(crate) nickname: String,
    pub(crate) wiimote_port: u8,
    pub(crate) ready: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetPlayBufferInput {
    pub(crate) pad_buffer: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetPlayStartInput {
    #[serde(default)]
    pub(crate) launch_dolphin_process: bool,
}

const PURE_RUST_WII_SHELL: &str = r#"<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <title>Menu Wii</title>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" href="/assets/css/universal.css">
  <link rel="stylesheet" href="/assets/tokens.css">
  <link rel="stylesheet" href="/assets/css/menu.css">
  <link rel="stylesheet" href="/assets/css/settings.css">
  <style>
    :root {
      color-scheme: light;
      --bg-dark: #0f172a;
      --wii-blue: #0284c7;
      --wii-teal: #54c5e5;
    }
    body, html {
      margin: 0;
      padding: 0;
      width: 100%;
      height: 100%;
      overflow: hidden;
      background: #000;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      user-select: none;
    }
    #wii-viewport {
      position: absolute;
      inset: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      background: radial-gradient(circle at center, #ffffff 0%, #e2e8f0 100%);
    }
    #wii-gpu-canvas {
      width: 100%;
      height: 100%;
      max-width: 1280px;
      max-height: 720px;
      aspect-ratio: 16 / 9;
      box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
    }
    .wii-status-bar {
      position: absolute;
      top: 16px;
      right: 24px;
      font-size: 14px;
      font-weight: 600;
      color: #64748b;
      letter-spacing: 0.05em;
    }
    .wii-boot-overlay {
      position: absolute;
      inset: 0;
      background: #ffffff;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      transition: opacity 0.8s ease;
      cursor: pointer;
      z-index: 50;
    }
    .wii-boot-overlay.hidden {
      opacity: 0;
      pointer-events: none;
    }
    .wii-logo {
      width: 180px;
      animation: pulse 2s infinite ease-in-out;
    }
    .wii-boot-text {
      margin-top: 32px;
      font-size: 16px;
      color: #94a3b8;
    }
    @keyframes pulse {
      0%, 100% { transform: scale(1); opacity: 0.9; }
      50% { transform: scale(1.05); opacity: 1; }
    }
    .wii-modal-backdrop {
      position: absolute;
      inset: 0;
      background: rgba(15, 23, 42, 0.7);
      backdrop-filter: blur(6px);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 100;
      transition: opacity 0.3s ease;
    }
    .wii-modal-backdrop.hidden {
      display: none;
    }
    .wii-card {
      background: #ffffff;
      border-radius: 20px;
      box-shadow: 0 20px 40px rgba(0, 0, 0, 0.25);
      width: 480px;
      max-width: 90vw;
      overflow: hidden;
      border: 3px solid #54c5e5;
      text-align: center;
    }
    .wii-card-header {
      background: linear-gradient(135deg, #0284c7 0%, #54c5e5 100%);
      color: #ffffff;
      padding: 16px 24px;
      font-size: 18px;
      font-weight: 700;
      letter-spacing: 0.05em;
    }
    .wii-card-body {
      padding: 24px;
      color: #334155;
    }
    .wii-code-badge {
      display: inline-block;
      margin: 16px 0;
      padding: 10px 24px;
      background: #f1f5f9;
      border-radius: 12px;
      border: 2px dashed #0284c7;
      font-size: 26px;
      font-weight: 800;
      letter-spacing: 0.1em;
      color: #0369a1;
    }
    .wii-card-actions {
      display: flex;
      gap: 12px;
      justify-content: center;
      margin-top: 16px;
    }
    .wii-btn {
      padding: 12px 28px;
      border-radius: 9999px;
      font-size: 15px;
      font-weight: 700;
      cursor: pointer;
      border: none;
      transition: all 0.2s ease;
    }
    .wii-btn-primary {
      background: #0284c7;
      color: #ffffff;
    }
    .wii-btn-primary:hover {
      background: #0369a1;
      transform: scale(1.03);
    }
    .wii-btn-secondary {
      background: #e2e8f0;
      color: #475569;
    }
    .wii-btn-secondary:hover {
      background: #cbd5e1;
    }
    .wii-home-trigger {
      position: absolute;
      bottom: 20px;
      width: 72px;
      height: 38px;
      background: #e2e8f0;
      border: 2px solid #94a3b8;
      border-radius: 19px;
      cursor: pointer;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 13px;
      font-weight: 800;
      color: #0284c7;
      box-shadow: 0 4px 6px rgba(0,0,0,0.1);
      transition: all 0.2s ease;
      z-index: 40;
    }
    .wii-home-trigger:hover {
      background: #0284c7;
      color: #ffffff;
      transform: scale(1.08);
    }
  </style>
</head>
<body>
  <div id="wii-viewport">
    <div class="wii-status-bar" id="wii-clock">--:--</div>
    <canvas id="wii-gpu-canvas" width="1280" height="720"></canvas>
    <div id="wii-boot-overlay" class="wii-boot-overlay">
      <img src="/assets/images/channel-wiilogo.png" class="wii-logo" alt="Wii">
      <div class="wii-boot-text">Appuyez sur le bouton A</div>
    </div>

    <!-- HOME Menu Button -->
    <div class="wii-home-trigger" id="wii-home-btn" title="Menu HOME">HOME</div>

    <!-- NetPlay Invite Modal -->
    <div id="wii-invite-modal" class="wii-modal-backdrop hidden">
      <div class="wii-card">
        <div class="wii-card-header">Connexion Wi-Fi Nintendo</div>
        <div class="wii-card-body">
          <p style="margin: 0; font-size: 16px;">Vous avez reçu une invitation multijoueur :</p>
          <div class="wii-code-badge" id="wii-invite-code-val">RDSP-42</div>
          <p style="margin: 0 0 16px 0; font-size: 14px; color: #64748b;" id="wii-invite-details">Salle en ligne — Prêt pour le combat</p>
          <div class="wii-card-actions">
            <button class="wii-btn wii-btn-primary" id="wii-btn-accept-invite">Rejoindre la salle</button>
            <button class="wii-btn wii-btn-secondary" id="wii-btn-close-invite">Retour au Menu</button>
          </div>
        </div>
      </div>
    </div>

    <!-- HOME Menu Modal -->
    <div id="wii-home-modal" class="wii-modal-backdrop hidden">
      <div class="wii-card" style="width: 520px;">
        <div class="wii-card-header">Menu HOME</div>
        <div class="wii-card-body">
          <div style="display: flex; justify-content: space-around; margin-bottom: 24px;">
            <div style="text-align: center;">
              <div style="font-weight: 700; color: #0284c7; margin-bottom: 4px;">P1</div>
              <div style="font-size: 18px;">🔋🔋🔋🔋</div>
            </div>
            <div style="text-align: center;">
              <div style="font-weight: 700; color: #ea580c; margin-bottom: 4px;">P2</div>
              <div style="font-size: 18px;">🔋🔋🔋⚪</div>
            </div>
            <div style="text-align: center;">
              <div style="font-weight: 700; color: #16a34a; margin-bottom: 4px;">P3</div>
              <div style="font-size: 18px;">🔋🔋⚪⚪</div>
            </div>
            <div style="text-align: center;">
              <div style="font-weight: 700; color: #ca8a04; margin-bottom: 4px;">P4</div>
              <div style="font-size: 18px;">🔋⚪⚪⚪</div>
            </div>
          </div>
          <div class="wii-card-actions">
            <button class="wii-btn wii-btn-primary" id="wii-home-menu-quit">Menu Wii</button>
            <button class="wii-btn wii-btn-secondary" id="wii-home-menu-reset">Réinitialiser</button>
            <button class="wii-btn wii-btn-secondary" id="wii-home-menu-close">Fermer</button>
          </div>
        </div>
      </div>
    </div>
  </div>
  <script type="module">
    // Nintendo Wii System Audio & Video Host
    let audioCtx = null;
    let bgmBuffer = null;
    let bgmSource = null;

    async function playSound(url) {
      if (!audioCtx) return;
      try {
        const res = await fetch(url);
        const buf = await res.arrayBuffer();
        const decoded = await audioCtx.decodeAudioData(buf);
        const src = audioCtx.createBufferSource();
        src.buffer = decoded;
        src.connect(audioCtx.destination);
        src.start();
      } catch (_) {}
    }

    async function initAudio() {
      if (audioCtx) return;
      audioCtx = new (window.AudioContext || window.webkitAudioContext)();
      try {
        await playSound('/assets/audio/startup.mp3');
        const bgmRes = await fetch('/assets/audio/bg-music.mp3');
        const bgmBuf = await bgmRes.arrayBuffer();
        bgmBuffer = await audioCtx.decodeAudioData(bgmBuf);
        playBgm();
      } catch (e) {
        console.warn("Audio init deferred:", e);
      }
    }

    function playBgm() {
      if (!audioCtx || !bgmBuffer) return;
      bgmSource = audioCtx.createBufferSource();
      bgmSource.buffer = bgmBuffer;
      bgmSource.loop = true;
      const gain = audioCtx.createGain();
      gain.gain.value = 0.4;
      bgmSource.connect(gain);
      gain.connect(audioCtx.destination);
      bgmSource.start();
    }

    function updateClock() {
      const now = new Date();
      const el = document.getElementById('wii-clock');
      if (el) {
        el.textContent = now.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' });
      }
    }
    setInterval(updateClock, 1000);
    updateClock();

    const overlay = document.getElementById('wii-boot-overlay');
    if (overlay) {
      overlay.addEventListener('click', async () => {
        await initAudio();
        overlay.classList.add('hidden');
      });
    }

    // HOME Menu Handlers
    const homeBtn = document.getElementById('wii-home-btn');
    const homeModal = document.getElementById('wii-home-modal');
    function toggleHomeMenu() {
      if (!homeModal) return;
      homeModal.classList.toggle('hidden');
      playSound('/assets/audio/button-select.mp3');
    }
    if (homeBtn) homeBtn.addEventListener('click', toggleHomeMenu);
    document.getElementById('wii-home-menu-close')?.addEventListener('click', () => {
      homeModal?.classList.add('hidden');
      playSound('/assets/audio/back.mp3');
    });
    document.getElementById('wii-home-menu-quit')?.addEventListener('click', () => {
      homeModal?.classList.add('hidden');
      playSound('/assets/audio/button-select.mp3');
    });
    document.getElementById('wii-home-menu-reset')?.addEventListener('click', () => {
      homeModal?.classList.add('hidden');
      playSound('/assets/audio/button-select.mp3');
      location.reload();
    });

    // Multiplayer Invite Detection: ?join=CODE or #join=CODE
    const urlParams = new URLSearchParams(window.location.search);
    const hashMatch = window.location.hash.match(/join=([^&]+)/);
    const inviteCode = urlParams.get('join') || (hashMatch ? hashMatch[1] : null);
    if (inviteCode) {
      const inviteModal = document.getElementById('wii-invite-modal');
      const codeVal = document.getElementById('wii-invite-code-val');
      if (inviteModal && codeVal) {
        codeVal.textContent = inviteCode.toUpperCase();
        inviteModal.classList.remove('hidden');
        document.getElementById('wii-btn-accept-invite')?.addEventListener('click', async () => {
          playSound('/assets/audio/button-select.mp3');
          inviteModal.classList.add('hidden');
          try {
            await fetch('/v1/netplay/join', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ host_address: location.hostname, port: 2626, nickname: 'Joueur Invité' })
            });
          } catch (_) {}
        });
        document.getElementById('wii-btn-close-invite')?.addEventListener('click', () => {
          playSound('/assets/audio/back.mp3');
          inviteModal.classList.add('hidden');
        });
      }
    }

    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' || e.key === 'Tab' || e.key === 'h' || e.key === 'H') {
        toggleHomeMenu();
      }
    });

    const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(`${wsProto}//${location.host}/ws`);
    ws.onmessage = (e) => {
      try {
        const msg = JSON.parse(e.data);
        console.log("Événement Système Wii :", msg);
      } catch (_) {}
    };
  </script>
</body>
</html>
"#;

#[allow(dead_code)]
const PURE_RUST_BT3_SHELL: &str = r#"<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8">
  <title>Dragon Ball Z: Budokai Tenkaichi 3</title>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    body, html {
      margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #000;
    }
    #gecko-canvas {
      width: 100vw; height: 100vh; display: block; object-fit: contain;
    }
  </style>
</head>
<body>
  <canvas id="gecko-canvas" width="1280" height="720"></canvas>
  <script type="module">
    console.log("Nintendo Wii — Démarrage du disque...");
  </script>
</body>
</html>
"#;

const WEB_INDEX: &str = PURE_RUST_WII_SHELL;
const WEB_TOKENS: &str = include_str!("../../../replica/design/tokens.css");

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_server(None).await
}

pub async fn run_server(bind_override: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let address: SocketAddr = bind_override
        .map(str::to_owned)
        .or_else(|| env::var("WII_OS_BIND").ok())
        .unwrap_or_else(|| "127.0.0.1:8780".to_owned())
        .parse()?;
    if let Some(token) = env::var_os("WII_OS_API_TOKEN") {
        let token = token.to_string_lossy();
        if token.len() < 32
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err("WII_OS_API_TOKEN must contain at least 32 URL-safe ASCII characters".into());
        }
    }
    if (!address.ip().is_loopback() || address.ip().is_unspecified())
        && env::var_os("WII_OS_API_TOKEN").is_none()
    {
        return Err("WII_OS_API_TOKEN is required when WII_OS_BIND is not loopback".into());
    }
    if (!address.ip().is_loopback() || address.ip().is_unspecified())
        && env::var("WII_OS_COOKIE_SECURE").as_deref() != Ok("1")
    {
        return Err("WII_OS_COOKIE_SECURE=1 is required for non-loopback binds; serve the app over HTTPS".into());
    }

    let (events, _) = broadcast::channel(256);
    let workspace = resolve_workspace_root();
    let (catalog_path, catalog) =
        load_workspace_catalog(&workspace).map_err(std::io::Error::other)?;

    let configured_database = env::var_os("WII_OS_DATABASE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| workspace.join("userland/data/wii-os.sqlite3"));
    let database_path = if configured_database.is_absolute() {
        configured_database
    } else {
        workspace.join(configured_database)
    };
    let legacy_room_store = env::var_os("WII_OS_ROOM_STORE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| workspace.join("userland/data/rooms.json"));
    let legacy_room_store = if legacy_room_store.is_absolute() {
        legacy_room_store
    } else {
        workspace.join(legacy_room_store)
    };

    let (database, stored_rooms) = RoomDatabase::open(database_path, legacy_room_store)
        .await
        .map_err(std::io::Error::other)?;
    let mut rooms: BTreeMap<String, Room> = stored_rooms
        .into_iter()
        .map(|room| (room.id.clone(), room))
        .collect();

    // Ensure a default NetPlay room exists for immediate Budokai Tenkaichi 3 / Wii Menu NetPlay testing
    if rooms.is_empty() {
        let invite_code = generate_room_invite_code("tenkaichi-3-eu-arena");
        let default_room = Room {
            id: "tenkaichi-3-eu-arena".to_string(),
            title_id: Some("0001000052445350".to_string()),
            players: 1,
            max_players: 4,
            transport: "dolphin-netplay-ffi".to_string(),
            channel_id: Some("budokai-tenkaichi-3-eu".to_string()),
            host_nickname: Some("Sangoku PAL".to_string()),
            host_address: Some("127.0.0.1".to_string()),
            port: Some(2626),
            pad_buffer: Some(6),
            game_digest: Some("RDSPAF-PAL-EU".to_string()),
            status: Some("lobby".to_string()),
            invite_code: Some(invite_code.clone()),
            invite_url: Some(format!("http://127.0.0.1:8780/?join={invite_code}")),
        };
        let _ = database.insert_room(default_room.clone()).await;
        rooms.insert(default_room.id.clone(), default_room);
    }

    let netplay = NetPlayManager::new(workspace.clone());
    prewarm_first_wii_disc(&catalog.channels, &workspace);

    let state = AppState {
        events,
        rooms: Arc::new(RwLock::new(rooms)),
        database,
        workspace: workspace.clone(),
        catalog_path,
        catalog: Arc::new(RwLock::new(catalog)),
        netplay,
    };

    let mcp_service = mcp::service(state.clone());
    let mut app = Router::new()
        .route("/", get(web_index))
        .route("/favicon.ico", get(serve_favicon))
        .route("/favicon.png", get(serve_favicon))
        .route("/assets/tokens.css", get(web_tokens))
        .route("/replica/design/tokens.css", get(web_tokens))
        .route("/auth/status", get(auth_status))
        .route(
            "/auth/session",
            post(create_session).delete(delete_session),
        )
        .route(
            "/healthz",
            get(|| async {
                Json(serde_json::json!({"status":"ok","service":"wii-os-control-plane","version":"4.3E"}))
            }),
        )
        .route("/v1/apps", get(list_apps))
        .route("/v1/apps/{app_id}/launch", post(launch_app))
        .route("/v1/settings", get(get_settings).put(update_settings))
        .route("/v1/sysconf", get(get_sysconf).put(update_sysconf))
        .route("/v1/channels/order", put(update_channel_order))
        .route("/v1/nand/summary", get(get_nand_summary))
        .route("/v1/nand/saves/{entry_id}/copy-sd", post(copy_save_sd))
        .route("/v1/nand/saves/{entry_id}", delete(delete_save))
        .route(
            "/v1/board/messages",
            get(list_board_messages).post(create_board_message),
        )
        .route("/v1/board/messages/{id}/read", post(mark_board_read))
        .route("/v1/board/messages/{id}", delete(remove_board_message))
        .route("/v1/rooms", get(list_rooms).post(create_room))
        .route(
            "/v1/rooms/{id}/messages",
            get(list_room_chat).post(post_room_chat),
        )
        .route("/v1/netplay/invite/{room_id}", get(netplay_invite))
        .route("/join/{code}", get(join_redirect))
        .route("/v1/netplay/status", get(netplay_status))
        .route("/v1/netplay/host", post(netplay_host))
        .route("/v1/netplay/join", post(netplay_join))
        .route("/v1/netplay/start", post(netplay_start))
        .route("/v1/netplay/stop", post(netplay_stop))
        .route("/v1/netplay/pad", post(netplay_pad))
        .route("/v1/netplay/buffer", post(netplay_buffer))
        .route("/v1/netplay/chat", post(netplay_chat))
        .route("/v1/packages", get(get_packages))
        .route("/v1/packages/build", post(build_package))
        .route("/v1/re/inspect/{slug}", get(inspect_re_manifest))
        .route("/v1/cdn/shenron/manifest", get(serve_shenron_cdn_manifest))
        .route("/cdn/shenron/manifest.json", get(serve_shenron_cdn_manifest))
        .route("/shenron", get(serve_shenron_redirect))
        .route("/shenron/", get(serve_shenron_redirect))
        .route("/shenron/index.html", get(serve_shenron_redirect))
        .route("/wii-files/games/{slug}", get(serve_wii_game_disc))
        .route("/cdn/shenron/games/{slug}", get(serve_wii_game_disc))
        .route("/games/budokai-tenkaichi-3", get(serve_bt3_fullscreen_game))
        .route("/games/budokai-tenkaichi-3/", get(serve_bt3_fullscreen_game))
        .route("/games/budokai-tenkaichi-3/index.html", get(serve_bt3_fullscreen_game))
        .route("/views/{*path}", get(serve_wii_view))
        .route("/v1/ws", get(websocket))
        .route("/menu.catalog.json", get(menu_catalog))
        .route("/v1/catalog", get(menu_catalog))
        .route("/wii-assets/{*path}", get(serve_wii_asset))
        .route("/assets/images/{*path}", get(serve_image_asset))
        .route("/assets/audio/{*path}", get(serve_audio_asset))
        .route("/assets/js/{*path}", get(serve_js_asset))
        .route("/assets/css/{*path}", get(serve_css_asset))
        .nest_service("/mcp", mcp_service)
        .with_state(state);

    let user_assets = workspace.join("userland/data/wii/assets");
    if user_assets.is_dir() {
        app = app.nest_service("/assets/user", ServeDir::new(user_assets));
    }
    let packages_dir = workspace.join("userland/build/packages");
    if tokio::fs::create_dir_all(&packages_dir).await.is_ok() {
        app = app.nest_service(
            "/packages",
            ServeDir::new(packages_dir).append_index_html_on_directories(true),
        );
    }
    let re_dir = workspace.join("userland/build/re");
    if re_dir.is_dir() {
        app = app.nest_service(
            "/re-assets",
            ServeDir::new(&re_dir).append_index_html_on_directories(true),
        );
        let bt3_re = re_dir.join("budokai-tenkaichi-3-eu");
        let audio_dir = bt3_re.join("bunre-pipeline/open_assets/audio");
        if audio_dir.is_dir() {
            app = app.nest_service("/cdn/shenron/audio", ServeDir::new(audio_dir));
        }
        let tex_dir = bt3_re.join("assets/textures");
        if tex_dir.is_dir() {
            app = app.nest_service("/cdn/shenron/textures", ServeDir::new(tex_dir));
        }
        let video_dir = bt3_re.join("assets/video");
        if video_dir.is_dir() {
            app = app.nest_service("/cdn/shenron/video", ServeDir::new(video_dir));
        }
        let frames_dir = bt3_re.join("frames");
        if frames_dir.is_dir() {
            app = app.nest_service("/cdn/shenron/frames", ServeDir::new(frames_dir));
        }
    }
    let cdn_packages_dir = workspace.join("userland/build/packages");
    if cdn_packages_dir.is_dir() {
        app = app.nest_service(
            "/cdn/shenron/packages",
            ServeDir::new(cdn_packages_dir).append_index_html_on_directories(true),
        );
    }
    let default_emulator_dir = workspace.join("crates/gecko-web");
    if default_emulator_dir.is_dir() && env::var_os("WII_OS_WEB_ROOT").is_none() {
        app = app.nest_service(
            "/emulator",
            ServeDir::new(default_emulator_dir).append_index_html_on_directories(true),
        );
    }

    if let Some(web_root) = env::var_os("WII_OS_WEB_ROOT").map(std::path::PathBuf::from) {
        let menu_root = web_root.join("menu");
        if menu_root.is_dir() {
            app = app.nest_service(
                "/menu",
                ServeDir::new(menu_root).append_index_html_on_directories(true),
            );
        }
        let emulator_root = web_root.join("emulator");
        if emulator_root.is_dir() {
            app = app.nest_service(
                "/emulator",
                ServeDir::new(emulator_root).append_index_html_on_directories(true),
            );
        }
        app = app.fallback_service(ServeDir::new(web_root).append_index_html_on_directories(true));
    } else {
        let frontend_dir = workspace.join("frontend");
        if frontend_dir.is_dir() {
            app = app.fallback_service(ServeDir::new(frontend_dir).append_index_html_on_directories(true));
        }
    }

    // Install layers after all routes and nested services so emulator assets
    // and fallback files receive the same isolation headers as the API.
    app = app
        .layer(middleware::from_fn(require_api_token))
        .layer(middleware::from_fn(add_security_headers));
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!("Wii OS BIOS 4.3E & NetPlay self-hosted binary listening on http://{address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Self-hosted pure-Rust COOP/COEP HTTP server for standalone WASM + WebGPU game packages.
pub async fn serve_wasm_package(
    package_dir: std::path::PathBuf,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let app = Router::new()
        .fallback_service(ServeDir::new(&package_dir).append_index_html_on_directories(true))
        .layer(middleware::from_fn(
            |request: Request, next: Next| async move {
                let mut response = next.run(request).await;
                let headers = response.headers_mut();
                headers.insert(
                    HeaderName::from_static("cross-origin-opener-policy"),
                    HeaderValue::from_static("same-origin"),
                );
                headers.insert(
                    HeaderName::from_static("cross-origin-embedder-policy"),
                    HeaderValue::from_static("require-corp"),
                );
                response
            },
        ));
    let listener = tokio::net::TcpListener::bind(address).await?;
    eprintln!(
        "Wii OS WASM+WebGPU package server listening on http://{address} ({})",
        package_dir.display()
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

use axum::http::HeaderName;

async fn serve_wii_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_embedded_or_disk(&state.workspace, &path).await
}

async fn serve_favicon(State(state): State<AppState>) -> Response {
    let ico_path = state.workspace.join("assets/favicon.ico");
    if let Ok(bytes) = tokio::fs::read(&ico_path).await {
        let mut response = Response::new(Body::from(bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("image/x-icon"),
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        );
        return response;
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}

async fn serve_image_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_embedded_or_disk(&state.workspace, &format!("images/{path}")).await
}

async fn serve_audio_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_embedded_or_disk(&state.workspace, &format!("audio/{path}")).await
}

async fn serve_js_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_embedded_or_disk(&state.workspace, &format!("js/{path}")).await
}

async fn serve_css_asset(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Response {
    serve_embedded_or_disk(&state.workspace, &format!("css/{path}")).await
}

async fn serve_embedded_or_disk(workspace: &std::path::Path, rel_path: &str) -> Response {
    if let Some(asset) = embedded_assets::lookup_embedded_asset(rel_path) {
        let mut response = Response::new(Body::from(asset.bytes));
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(asset.content_type),
        );
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        );
        return response;
    }
    let clean = rel_path.trim_start_matches('/');
    if clean.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let disk_path = workspace.join("frontend/assets").join(clean);
    if let Ok(bytes) = tokio::fs::read(&disk_path).await {
        let mime = if clean.ends_with(".png") {
            "image/png"
        } else if clean.ends_with(".jpg") || clean.ends_with(".jpeg") {
            "image/jpeg"
        } else if clean.ends_with(".svg") {
            "image/svg+xml"
        } else if clean.ends_with(".mp3") {
            "audio/mpeg"
        } else if clean.ends_with(".css") {
            "text/css; charset=utf-8"
        } else if clean.ends_with(".js") {
            "text/javascript; charset=utf-8"
        } else if clean.ends_with(".wasm") {
            "application/wasm"
        } else {
            "application/octet-stream"
        };
        let mut response = Response::new(Body::from(bytes));
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
        return response;
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn web_index(State(state): State<AppState>) -> Response {
    match tokio::fs::read(state.workspace.join("frontend/index.html")).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => Html(WEB_INDEX).into_response(),
    }
}

async fn web_tokens() -> Response {
    static_asset("text/css; charset=utf-8", WEB_TOKENS, "no-cache")
}

fn static_asset(content_type: &'static str, content: &'static str, cache: &'static str) -> Response {
    let mut response = Response::new(Body::from(content));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache),
    );
    response
}

async fn auth_status(headers: HeaderMap) -> Json<serde_json::Value> {
    let required = env::var_os("WII_OS_API_TOKEN").is_some();
    let authenticated = !required || cookie_token(&headers).is_some_and(|supplied| {
        env::var("WII_OS_API_TOKEN")
            .ok()
            .is_some_and(|expected| constant_time_eq(supplied.as_bytes(), expected.as_bytes()))
    });
    Json(serde_json::json!({"required": required, "authenticated": authenticated}))
}

#[derive(Deserialize)]
struct SessionRequest {
    token: String,
}

async fn create_session(Json(input): Json<SessionRequest>) -> Response {
    let Some(expected) = env::var("WII_OS_API_TOKEN").ok() else {
        return StatusCode::NO_CONTENT.into_response();
    };
    if !constant_time_eq(input.token.as_bytes(), expected.as_bytes()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"secret incorrect"})),
        )
            .into_response();
    }
    let secure = if env::var("WII_OS_COOKIE_SECURE").as_deref() == Ok("1") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!("wii_os_token={expected}; Path=/; Max-Age=28800; HttpOnly; SameSite=Strict{secure}");
    let mut response = StatusCode::NO_CONTENT.into_response();
    match HeaderValue::from_str(&cookie) {
        Ok(value) => {
            response.headers_mut().insert(header::SET_COOKIE, value);
            response
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn delete_session() -> Response {
    let secure = if env::var("WII_OS_COOKIE_SECURE").as_deref() == Ok("1") {
        "; Secure"
    } else {
        ""
    };
    let cookie = format!("wii_os_token=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{secure}");
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == "wii_os_token").then(|| value.to_owned())
        })
}

async fn add_security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(header::REFERRER_POLICY, HeaderValue::from_static("strict-origin-when-cross-origin"));
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(HeaderName::from_static("cross-origin-opener-policy"), HeaderValue::from_static("same-origin"));
    headers.insert(HeaderName::from_static("cross-origin-embedder-policy"), HeaderValue::from_static("require-corp"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'self'; img-src 'self' data: blob:; media-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; connect-src 'self' ws: wss:; object-src 'none'; base-uri 'self'; frame-ancestors 'self'"),
    );
    response
}

async fn require_api_token(request: Request, next: Next) -> axum::response::Response {
    let path = request.uri().path();
    let protected = path.starts_with("/v1/") || path == "/mcp" || path.starts_with("/mcp/");
    if !protected {
        return next.run(request).await;
    }
    let expected = env::var("WII_OS_API_TOKEN").ok();
    let Some(expected) = expected else {
        let loopback = env::var("WII_OS_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8780".to_owned())
            .parse::<SocketAddr>()
            .is_ok_and(|address| address.ip().is_loopback());
        if loopback {
            return next.run(request).await;
        }
        return StatusCode::FORBIDDEN.into_response();
    };
    let bearer = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let cookie = cookie_token(request.headers());
    let supplied = bearer.unwrap_or_else(|| cookie.as_deref().unwrap_or_default());
    if !constant_time_eq(supplied.as_bytes(), expected.as_bytes()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error":"valid bearer token required"})),
        )
            .into_response();
    }
    next.run(request).await
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

fn app_wad_environment(id: &str) -> Option<&'static str> {
    match id {
        "system-menu" => Some("WII_OS_SYSTEM_MENU_WAD"),
        "mii-channel" => Some("WII_OS_MII_CHANNEL_WAD"),
        "shop-channel" => Some("WII_OS_SHOP_CHANNEL_WAD"),
        _ => None,
    }
}

async fn menu_catalog(State(state): State<AppState>) -> Json<MenuCatalog> {
    Json(state.catalog.read().await.clone())
}

pub(crate) async fn list_apps(State(state): State<AppState>) -> Json<Vec<AppInfo>> {
    let catalog = state.catalog.read().await;
    let dolphin_ready = resolve_dolphin_executable(&state.workspace).is_some();
    Json(
        catalog
            .channels
            .iter()
            .map(|channel| {
                let id = channel
                    .app_id
                    .as_deref()
                    .unwrap_or(&channel.id)
                    .to_owned();
                let content_ready = channel_content_ready(&state.workspace, channel);
                AppInfo {
                    id,
                    channel_id: channel.id.clone(),
                    title: channel.title.clone(),
                    title_id: channel.title_id.clone(),
                    content_kind: channel.content_kind,
                    icon: channel.icon.clone(),
                    blocks: channel.blocks.unwrap_or(16),
                    publisher: channel
                        .publisher
                        .clone()
                        .unwrap_or_else(|| "Nintendo / Wii OS".to_string()),
                    players: channel.players.unwrap_or(1),
                    content_ready: content_ready && dolphin_ready,
                }
            })
            .collect(),
    )
}

pub(crate) async fn get_settings(State(state): State<AppState>) -> impl IntoResponse {
    let backend = match env::var("WII_OS_DOLPHIN_VIDEO_BACKEND") {
        Ok(value) => Some(value),
        Err(_) => match state.database.setting("dolphin.videoBackend").await {
            Ok(value) => value.or_else(default_video_backend),
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error":error})),
                )
                    .into_response();
            }
        },
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({"videoBackend":backend})),
    )
        .into_response()
}

pub(crate) async fn update_settings(
    State(state): State<AppState>,
    Json(settings): Json<AppSettings>,
) -> impl IntoResponse {
    let Some(backend) = settings.video_backend else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"videoBackend is required"})),
        )
            .into_response();
    };
    let Some(backend) = normalize_video_backend(&backend) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"unsupported Dolphin video backend"})),
        )
            .into_response();
    };
    match state
        .database
        .set_setting("dolphin.videoBackend", &backend)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"videoBackend":backend})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

pub(crate) async fn get_sysconf(State(state): State<AppState>) -> impl IntoResponse {
    match state.database.get_sysconf().await {
        Ok(sysconf) => (StatusCode::OK, Json(sysconf)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

pub(crate) async fn update_sysconf(
    State(state): State<AppState>,
    Json(mut sysconf): Json<WiiSysconf>,
) -> impl IntoResponse {
    if let Some(norm) = normalize_video_backend(&sysconf.video_backend) {
        sysconf.video_backend = norm.to_string();
    }
    sysconf.sensor_bar_sensitivity = sysconf.sensor_bar_sensitivity.clamp(1, 5);
    match state.database.save_sysconf(&sysconf).await {
        Ok(()) => {
            let _ = state.events.send(ServerEvent {
                room: "system".to_string(),
                event: "sysconf_updated".to_string(),
                detail: Some(sysconf.console_nickname.clone()),
                payload: serde_json::to_value(&sysconf).ok(),
            });
            (StatusCode::OK, Json(sysconf)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

pub(crate) async fn update_channel_order(
    State(state): State<AppState>,
    Json(input): Json<ChannelMoveInput>,
) -> impl IntoResponse {
    let mut catalog = state.catalog.write().await;
    let target_page = input.preferred_page.min(19);
    let target_slot = input.preferred_slot.min(11);

    // If another channel already sits at (target_page, target_slot), swap its coordinates
    let prev_coords = catalog
        .channels
        .iter()
        .find(|c| c.id == input.channel_id)
        .map(|c| (c.preferred_page, c.preferred_slot));

    let Some((old_page, old_slot)) = prev_coords else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"channel not found"})),
        )
            .into_response();
    };

    for ch in &mut catalog.channels {
        if ch.id == input.channel_id {
            ch.preferred_page = Some(target_page);
            ch.preferred_slot = Some(target_slot);
        } else if ch.preferred_page == Some(target_page) && ch.preferred_slot == Some(target_slot) {
            ch.preferred_page = old_page;
            ch.preferred_slot = old_slot;
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&*catalog) {
        let _ = tokio::fs::write(&state.catalog_path, json).await;
    }

    let snapshot = catalog.clone();
    drop(catalog);

    let _ = state.events.send(ServerEvent {
        room: "system".to_string(),
        event: "channel_reordered".to_string(),
        detail: Some(input.channel_id),
        payload: serde_json::to_value(&snapshot).ok(),
    });

    (StatusCode::OK, Json(snapshot)).into_response()
}

pub(crate) async fn get_nand_summary(State(state): State<AppState>) -> impl IntoResponse {
    let workspace = state.workspace.clone();
    let summary = tokio::task::spawn_blocking(move || inspect_nand_summary(&workspace))
        .await
        .unwrap_or_else(|_| inspect_nand_summary(&state.workspace));
    (StatusCode::OK, Json(summary)).into_response()
}

pub(crate) async fn copy_save_sd(
    State(state): State<AppState>,
    Path(entry_id): Path<String>,
) -> impl IntoResponse {
    match copy_nand_save_to_sd(&state.workspace, &entry_id) {
        Ok(dest) => (
            StatusCode::OK,
            Json(serde_json::json!({"copied": true, "entryId": entry_id, "sdPath": dest})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn delete_save(
    State(state): State<AppState>,
    Path(entry_id): Path<String>,
) -> impl IntoResponse {
    match delete_nand_save_entry(&state.workspace, &entry_id) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"deleted": true, "entryId": entry_id})),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn list_board_messages(State(state): State<AppState>) -> impl IntoResponse {
    match state.database.list_board_messages().await {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn create_board_message(
    State(state): State<AppState>,
    Json(input): Json<NewBoardMessage>,
) -> impl IntoResponse {
    if input.title.trim().is_empty() || input.body.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "title and body are required"})),
        )
            .into_response();
    }
    match state.database.insert_board_message(input).await {
        Ok(msg) => {
            let _ = state.events.send(ServerEvent {
                room: "system".to_string(),
                event: "board_message_posted".to_string(),
                detail: Some(msg.title.clone()),
                payload: serde_json::to_value(&msg).ok(),
            });
            (StatusCode::CREATED, Json(msg)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn mark_board_read(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.database.mark_board_message_read(id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"read": true}))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn remove_board_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.database.delete_board_message(id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn launch_app(
    State(state): State<AppState>,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let catalog = state.catalog.read().await;
    let Some(channel) = catalog
        .channels
        .iter()
        .find(|channel| {
            channel.app_id.as_deref() == Some(app_id.as_str()) || channel.id == app_id
        })
        .cloned()
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"unknown app"})),
        )
            .into_response();
    };
    drop(catalog);

    let title = channel.title.clone();
    if !channel_content_ready(&state.workspace, &channel) {
        let message = if channel.content_kind == wii_menu_core::ContentKind::WiiNandTitle {
            format!(
                "title {} is not installed in the configured Dolphin NAND profile",
                channel.title_id.as_deref().unwrap_or("<missing titleId>")
            )
        } else {
            app_wad_environment(&app_id)
                .map(|key| {
                    format!(
                        "configure {key} or a valid local contentPath in the Wii OS menu catalog"
                    )
                })
                .unwrap_or_else(|| {
                    "configure a valid local WAD/DOL contentPath in the Wii OS menu catalog"
                        .to_owned()
                })
        };
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":message})),
        )
            .into_response();
    }

    let backend = match env::var("WII_OS_DOLPHIN_VIDEO_BACKEND") {
        Ok(value) => Some(value),
        Err(_) => match state.database.setting("dolphin.videoBackend").await {
            Ok(value) => value.or_else(default_video_backend),
            Err(error) => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({"error":error})),
                )
                    .into_response();
            }
        },
    };
    match launch_catalog_channel_with_backend(&channel, &state.workspace, backend.as_deref()) {
        Ok(pid) => {
            // Automatically record Play History on the Wii Message Board (`iplBoard`)
            let _ = state
                .database
                .insert_board_message(NewBoardMessage {
                    date_key: Some("Aujourd'hui".to_string()),
                    kind: Some("play-history".to_string()),
                    title: format!("Session lancée : {title}"),
                    author: Some("Bureau Wii".to_string()),
                    body: format!(
                        "Titre : {title}\nPID Dolphin : {pid}\nBackend vidéo : {}",
                        backend.as_deref().unwrap_or("D3D12")
                    ),
                    title_id: channel.title_id.clone(),
                    duration_minutes: Some(1),
                })
                .await;

            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({"app":app_id,"title":title,"pid":pid})),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":error})),
        )
            .into_response(),
    }
}

fn default_video_backend() -> Option<String> {
    cfg!(target_os = "windows").then(|| "D3D12".to_owned())
}

fn channel_boot_path(
    workspace: &std::path::Path,
    channel: &wii_menu_core::Channel,
) -> Option<std::path::PathBuf> {
    if channel.content_kind == wii_menu_core::ContentKind::WiiWad
        && let Some(key) = channel.app_id.as_deref().and_then(app_wad_environment)
        && let Some(path) = env::var_os(key).map(std::path::PathBuf::from)
    {
        if path.is_file() {
            return Some(path);
        }
        if let Ok(resolved) = resolve_workspace_file(workspace, &path) {
            return Some(resolved);
        }
    }
    let path = std::path::Path::new(channel.content_path.as_deref()?);
    let valid_extension = match channel.content_kind {
        wii_menu_core::ContentKind::WiiDisc => ["rvz", "iso", "gcm", "gcz", "wia", "wbfs"]
            .iter()
            .any(|extension| {
                path.extension()
                    .is_some_and(|actual| actual.to_string_lossy().eq_ignore_ascii_case(extension))
            }),
        wii_menu_core::ContentKind::WiiWad => path
            .extension()
            .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("wad")),
        wii_menu_core::ContentKind::Homebrew => path
            .extension()
            .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("dol")),
        _ => false,
    };
    if !valid_extension {
        return None;
    }
    resolve_workspace_file(workspace, path).ok()
}

fn channel_content_ready(workspace: &std::path::Path, channel: &wii_menu_core::Channel) -> bool {
    if channel.content_kind == wii_menu_core::ContentKind::WiiNandTitle {
        let Some(title_id) = channel
            .title_id
            .as_deref()
            .filter(|id| id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        else {
            return false;
        };
        let profile = wii_menu_runtime::resolve_dolphin_profile_dir(workspace);
        let (high, low) = title_id.split_at(8);
        return profile
            .join("Wii/title")
            .join(high)
            .join(low)
            .join("content/title.tmd")
            .is_file();
    }
    let required_files_ready = channel.required_files.iter().all(|required| {
        resolve_workspace_file(workspace, std::path::Path::new(&required.content_path)).is_ok()
    });
    required_files_ready
        && matches!(
            channel.content_kind,
            wii_menu_core::ContentKind::WiiDisc
                | wii_menu_core::ContentKind::WiiWad
                | wii_menu_core::ContentKind::Homebrew
        )
        && channel_boot_path(workspace, channel).is_some()
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

// ============================================================================
// ROOMS & LOBBY CHAT ENDPOINTS
// ============================================================================

pub(crate) async fn list_rooms(State(state): State<AppState>) -> Json<Vec<Room>> {
    Json(state.rooms.read().await.values().cloned().collect())
}

pub(crate) async fn create_room(
    State(state): State<AppState>,
    Json(request): Json<CreateRoom>,
) -> impl IntoResponse {
    if request.id.trim().is_empty() || request.id.len() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"room id must contain 1-64 characters"})),
        )
            .into_response();
    }
    let transport = request
        .transport
        .unwrap_or_else(|| "dolphin-netplay-ffi".to_owned());
    if !matches!(
        transport.as_str(),
        "dolphin-netplay-external" | "dolphin-netplay-ffi" | "webrtc-datachannel"
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"unsupported transport"})),
        )
            .into_response();
    }
    let channel_id = request
        .channel_id
        .clone()
        .unwrap_or_else(|| "budokai-tenkaichi-3-eu".to_string());
    let game_digest = state
        .netplay
        .resolve_channel_and_digest(&channel_id)
        .ok()
        .map(|(_, _, digest)| digest);

    let invite_code = generate_room_invite_code(&request.id);
    let invite_url = format!("http://127.0.0.1:8780/?join={invite_code}");

    let room = Room {
        id: request.id.clone(),
        title_id: request.title_id,
        players: 1,
        max_players: request.max_players.unwrap_or(4).clamp(1, 16),
        transport,
        channel_id: Some(channel_id),
        host_nickname: Some(
            request
                .host_nickname
                .unwrap_or_else(|| "Hôte Wii".to_string()),
        ),
        host_address: Some(
            request
                .host_address
                .unwrap_or_else(|| "127.0.0.1".to_string()),
        ),
        port: Some(request.port.unwrap_or(2626)),
        pad_buffer: Some(request.pad_buffer.unwrap_or(6)),
        game_digest,
        status: Some("lobby".to_string()),
        invite_code: Some(invite_code),
        invite_url: Some(invite_url),
    };
    let mut rooms = state.rooms.write().await;
    if let Err(error) = state.database.insert_room(room.clone()).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":format!("could not persist room in SQLite: {error}")})),
        )
            .into_response();
    }
    rooms.insert(room.id.clone(), room.clone());
    drop(rooms);
    let _ = state.events.send(ServerEvent {
        room: room.id.clone(),
        event: "room_created".to_owned(),
        detail: room.channel_id.clone(),
        payload: serde_json::to_value(&room).ok(),
    });
    (StatusCode::CREATED, Json(room)).into_response()
}

pub(crate) async fn list_room_chat(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> impl IntoResponse {
    match state.database.list_room_messages(&room_id).await {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn post_room_chat(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Json(input): Json<RoomChatInput>,
) -> impl IntoResponse {
    if input.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"message cannot be empty"})),
        )
            .into_response();
    }
    let _ = state.netplay.record_chat(&input.sender, &input.message);
    match state
        .database
        .insert_room_message(&room_id, &input.sender, &input.message)
        .await
    {
        Ok(msg) => {
            let _ = state.events.send(ServerEvent {
                room: room_id,
                event: "room_chat".to_string(),
                detail: Some(format!("{}: {}", msg.sender, msg.message)),
                payload: serde_json::to_value(&msg).ok(),
            });
            (StatusCode::CREATED, Json(msg)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoomInviteResponse {
    pub room_id: String,
    pub invite_code: String,
    pub invite_url: String,
    pub short_code: String,
    pub qr_data: String,
    pub channel_id: Option<String>,
    pub max_players: u32,
}

pub(crate) async fn netplay_invite(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let rooms = state.rooms.read().await;
    let room = rooms.values().find(|r| {
        r.id.eq_ignore_ascii_case(&room_id)
            || r.invite_code.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(&room_id))
            || generate_room_invite_code(&r.id).eq_ignore_ascii_case(&room_id)
    });
    let Some(room) = room else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Salle multijoueur '{room_id}' introuvable")})),
        )
            .into_response();
    };

    let host_header = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1:8780");
    let code = room
        .invite_code
        .clone()
        .unwrap_or_else(|| generate_room_invite_code(&room.id));
    let invite_url = format!("http://{host_header}/?join={code}");

    (
        StatusCode::OK,
        Json(RoomInviteResponse {
            room_id: room.id.clone(),
            invite_code: code.clone(),
            invite_url: invite_url.clone(),
            short_code: code,
            qr_data: invite_url,
            channel_id: room.channel_id.clone(),
            max_players: room.max_players,
        }),
    )
        .into_response()
}

pub(crate) async fn join_redirect(
    Path(code): Path<String>,
) -> impl IntoResponse {
    let target = format!("/?join={code}");
    (
        StatusCode::TEMPORARY_REDIRECT,
        [(header::LOCATION, target)],
        (),
    )
}

// ============================================================================
// NETPLAY ENGINE ENDPOINTS (`/v1/netplay/*`)
// ============================================================================

pub(crate) async fn netplay_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.netplay.status() {
        Ok(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_host(
    State(state): State<AppState>,
    Json(req): Json<NetPlayHostRequest>,
) -> impl IntoResponse {
    match state.netplay.host_session(req) {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: snapshot
                    .room_id
                    .clone()
                    .unwrap_or_else(|| "netplay".to_string()),
                event: "netplay_hosted".to_string(),
                detail: snapshot.game_title.clone(),
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_join(
    State(state): State<AppState>,
    Json(req): Json<NetPlayJoinRequest>,
) -> impl IntoResponse {
    match state.netplay.join_session(req) {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: snapshot
                    .room_id
                    .clone()
                    .unwrap_or_else(|| "netplay".to_string()),
                event: "netplay_joined".to_string(),
                detail: snapshot.game_title.clone(),
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_start(
    State(state): State<AppState>,
    Json(input): Json<NetPlayStartInput>,
) -> impl IntoResponse {
    match state.netplay.start_game(input.launch_dolphin_process) {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: snapshot
                    .room_id
                    .clone()
                    .unwrap_or_else(|| "netplay".to_string()),
                event: "netplay_started".to_string(),
                detail: snapshot.game_title.clone(),
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_stop(State(state): State<AppState>) -> impl IntoResponse {
    match state.netplay.stop_session() {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: "netplay".to_string(),
                event: "netplay_stopped".to_string(),
                detail: None,
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_pad(
    State(state): State<AppState>,
    Json(input): Json<NetPlayPadInput>,
) -> impl IntoResponse {
    match state
        .netplay
        .assign_wiimote_port(&input.nickname, input.wiimote_port, input.ready)
    {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: "netplay".to_string(),
                event: "netplay_pad_updated".to_string(),
                detail: Some(format!("{} -> P{}", input.nickname, input.wiimote_port)),
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_buffer(
    State(state): State<AppState>,
    Json(input): Json<NetPlayBufferInput>,
) -> impl IntoResponse {
    match state.netplay.set_pad_buffer(input.pad_buffer) {
        Ok(snapshot) => (StatusCode::OK, Json(snapshot)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub(crate) async fn netplay_chat(
    State(state): State<AppState>,
    Json(input): Json<RoomChatInput>,
) -> impl IntoResponse {
    match state.netplay.record_chat(&input.sender, &input.message) {
        Ok(snapshot) => {
            let _ = state.events.send(ServerEvent {
                room: "netplay".to_string(),
                event: "netplay_chat".to_string(),
                detail: Some(format!("{}: {}", input.sender, input.message)),
                payload: serde_json::to_value(&snapshot).ok(),
            });
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

// ============================================================================
// GAME PACKAGING ENDPOINTS (`WASM` & `EXE`)
// ============================================================================

pub(crate) async fn get_packages(State(state): State<AppState>) -> impl IntoResponse {
    let workspace = state.workspace.clone();
    let packages = tokio::task::spawn_blocking(move || list_built_packages(&workspace))
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(packages)).into_response()
}

pub(crate) async fn build_package(
    State(state): State<AppState>,
    Json(req): Json<PackageRequest>,
) -> impl IntoResponse {
    let workspace = state.workspace.clone();
    let req_clone = req.clone();
    let result = tokio::task::spawn_blocking(move || package_game(&workspace, &req_clone))
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);

    match result {
        Ok(artifact) => {
            let _ = state
                .database
                .insert_board_message(NewBoardMessage {
                    date_key: Some("Aujourd'hui".to_string()),
                    kind: Some("system".to_string()),
                    title: format!(
                        "Package {} généré : {}",
                        artifact.format.to_uppercase(),
                        artifact.title
                    ),
                    author: Some("Studio Packaging Wii OS".to_string()),
                    body: format!(
                        "Format : {}\nDossier : {}\nEntrée : {}\nSHA-256 : {}\nCommande : {}",
                        artifact.format.to_uppercase(),
                        artifact.output_dir,
                        artifact.entrypoint_path,
                        artifact.sha256,
                        artifact.launch_command
                    ),
                    title_id: artifact.title_id.clone(),
                    duration_minutes: Some(0),
                })
                .await;

            let _ = state.events.send(ServerEvent {
                room: "system".to_string(),
                event: "package_built".to_string(),
                detail: Some(artifact.package_id.clone()),
                payload: serde_json::to_value(&artifact).ok(),
            });
            (StatusCode::CREATED, Json(artifact)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

// ============================================================================
// REAL-TIME WEBSOCKET (`/v1/ws`)
// ============================================================================

async fn websocket(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| serve_socket(socket, state))
}

async fn serve_socket(mut socket: WebSocket, state: AppState) {
    let mut events = state.events.subscribe();
    let mut joined_room: Option<String> = None;
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Ping(payload))) => if socket.send(Message::Pong(payload)).await.is_err() { break; },
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                Some(Ok(Message::Text(text))) => {
                    let request = serde_json::from_str::<ClientMessage>(&text);
                    let result = match request {
                        Ok(ClientMessage::JoinRoom { room }) => {
                            if joined_room.as_deref() == Some(room.as_str()) { continue; }
                            if let Some(old_room) = joined_room.take() {
                                let _ = update_room_members(&state, &old_room, false).await;
                            }
                            match update_room_members(&state, &room, true).await {
                                Ok(()) => { joined_room = Some(room); Ok(()) },
                                Err(error) => Err(error),
                            }
                        }
                        Ok(ClientMessage::LeaveRoom { room }) => {
                            if joined_room.as_deref() == Some(room.as_str()) {
                                joined_room = None;
                                update_room_members(&state, &room, false).await
                            } else {
                                Ok(())
                            }
                        }
                        Ok(ClientMessage::RoomChat { room, sender, message }) => {
                            let _ = state.netplay.record_chat(&sender, &message);
                            match state.database.insert_room_message(&room, &sender, &message).await {
                                Ok(msg) => {
                                    let _ = state.events.send(ServerEvent {
                                        room,
                                        event: "room_chat".to_string(),
                                        detail: Some(format!("{}: {}", msg.sender, msg.message)),
                                        payload: serde_json::to_value(&msg).ok(),
                                    });
                                    Ok(())
                                }
                                Err(err) => Err(err),
                            }
                        }
                        Ok(ClientMessage::PadAssign { room, nickname, wiimote_port, ready }) => {
                            match state.netplay.assign_wiimote_port(&nickname, wiimote_port, ready) {
                                Ok(snapshot) => {
                                    let _ = state.events.send(ServerEvent {
                                        room,
                                        event: "netplay_pad_updated".to_string(),
                                        detail: Some(format!("{nickname} -> P{wiimote_port}")),
                                        payload: serde_json::to_value(&snapshot).ok(),
                                    });
                                    Ok(())
                                }
                                Err(err) => Err(err),
                            }
                        }
                        Ok(ClientMessage::WebrtcSignal { room, sender, signal }) => {
                            let _ = state.events.send(ServerEvent {
                                room,
                                event: "webrtc_signal".to_string(),
                                detail: Some(sender),
                                payload: Some(signal),
                            });
                            Ok(())
                        }
                        Err(error) => Err(format!("invalid websocket message: {error}")),
                    };
                    if let Err(error) = result {
                        let response = serde_json::json!({"type":"error","detail":error}).to_string();
                        if socket.send(Message::Text(response.into())).await.is_err() { break; }
                    }
                },
                Some(Ok(Message::Binary(_))) | Some(Ok(Message::Pong(_))) => {},
            },
            event = events.recv() => match event {
                Ok(event) => if let Ok(json) = serde_json::to_string(&event) {
                    if socket.send(Message::Text(json.into())).await.is_err() { break; }
                },
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    if let Some(room) = joined_room {
        let _ = update_room_members(&state, &room, false).await;
    }
}

async fn update_room_members(state: &AppState, room_id: &str, join: bool) -> Result<(), String> {
    let mut rooms = state.rooms.write().await;
    let Some(room) = rooms.get_mut(room_id) else {
        return Err("room does not exist".to_owned());
    };
    if join && room.players >= room.max_players {
        return Err("room is full".to_owned());
    }
    if join {
        room.players += 1;
    } else {
        room.players = room.players.saturating_sub(1);
    }
    let event = ServerEvent {
        room: room.id.clone(),
        event: if join { "player_joined" } else { "player_left" }.to_owned(),
        detail: Some(format!("players={}", room.players)),
        payload: serde_json::to_value(&*room).ok(),
    };
    drop(rooms);
    let _ = state.events.send(event);
    Ok(())
}

async fn inspect_re_manifest(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Response {
    let clean_slug = slug.trim_end_matches(".rvz");
    let re_root = state.workspace.join("userland/build/re").join(clean_slug);
    let manifest_path = re_root.join("manifest.json");
    let bunre_path = re_root.join("bunre-pipeline/verification_summary.json");
    let smoke_path = re_root.join("frames/smoke_telemetry.json");

    let manifest_val = tokio::fs::read_to_string(&manifest_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    let bunre_val = tokio::fs::read_to_string(&bunre_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    let smoke_val = tokio::fs::read_to_string(&smoke_path)
        .await
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());

    Json(serde_json::json!({
        "slug": clean_slug,
        "status": if manifest_val.is_some() { "ready" } else { "not_extracted" },
        "re_manifest": manifest_val,
        "bun_re_engine_pipeline": bunre_val,
        "webgpu_smoke_telemetry": smoke_val,
        "emulator_url": format!("/emulator/index.html?game=/wii-files/games/{clean_slug}.rvz&autoboot=1"),
    }))
    .into_response()
}

async fn serve_wii_game_disc(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    req: Request,
) -> Response {
    let clean_id = slug.trim_end_matches(".rvz");
    let catalog = state.catalog.read().await;
    let channel_opt = catalog
        .channels
        .iter()
        .find(|ch| ch.id == clean_id || slug.contains(&ch.id))
        .cloned();
    drop(catalog);

    let disc_path = if let Some(ch) = channel_opt {
        ch.content_path
            .as_deref()
            .and_then(|p| resolve_workspace_file(&state.workspace, std::path::Path::new(p)).ok())
    } else {
        let candidate = state
            .workspace
            .join("userland/data/wii/roms/Dragon-Ball-Z-Budokai-Tenkaichi-3-Europe-En-Fr-De-Es-It.rvz");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    };

    let Some(path) = disc_path else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Wii disc not found for {slug}")})),
        )
            .into_response();
    };

    match tower_http::services::ServeFile::new(path).try_call(req).await {
        Ok(res) => res.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn serve_shenron_redirect() -> Response {
    axum::response::Redirect::temporary("/games/budokai-tenkaichi-3").into_response()
}

async fn serve_bt3_fullscreen_game(State(state): State<AppState>) -> Response {
    match tokio::fs::read(state.workspace.join("frontend/games/budokai-tenkaichi-3/index.html")).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Game page not installed").into_response(),
    }
}

async fn serve_wii_view(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    // View names are a single local HTML filename; never interpolate a request
    // path into markup or allow it to escape the templates directory.
    if !valid_view_filename(&path) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    match tokio::fs::read(state.workspace.join("frontend/views").join(path)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn valid_view_filename(path: &str) -> bool {
    path.ends_with(".html")
        && !path.contains("..")
        && path.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
}

#[cfg(test)]
mod view_route_tests {
    use super::valid_view_filename;

    #[test]
    fn accepts_local_template_names_and_rejects_traversal() {
        assert!(valid_view_filename("menu.html"));
        assert!(valid_view_filename("settings-main.html"));
        for invalid in ["../menu.html", "a/../menu.html", "..\\menu.html", "menu.js", "<script>.html"] {
            assert!(!valid_view_filename(invalid), "accepted {invalid}");
        }
    }
}

async fn serve_shenron_cdn_manifest(State(state): State<AppState>) -> Response {
    let bt3_re = state.workspace.join("userland/build/re/budokai-tenkaichi-3-eu");
    let audio_dir = bt3_re.join("bunre-pipeline/open_assets/audio");
    let mut audio_tracks = Vec::new();
    if let Ok(mut rd) = tokio::fs::read_dir(&audio_dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".wav") {
                    audio_tracks.push(name.to_string());
                }
            }
        }
    }
    audio_tracks.sort();

    let packages = list_built_packages(&state.workspace);
    Json(serde_json::json!({
        "cdn": "Shenron Global Asset & Game Streaming CDN",
        "version": "2026.4.3E",
        "edges": [
            {
                "id": "hostinger-prod",
                "host": "72.60.186.43",
                "role": "Primary Public Edge & WebGPU COOP/COEP Origin",
                "status": "online"
            },
            {
                "id": "shenron-node",
                "host": "10.200.77.3",
                "role": "WireGuard High-Capacity RVZ & RE Storage Node",
                "status": "online"
            }
        ],
        "games": [
            {
                "id": "budokai-tenkaichi-3-eu",
                "titleId": "RDSPAF",
                "title": "Dragon Ball Z : Budokai Tenkaichi 3 (PAL Europe EN/FR/DE/ES/IT)",
                "streamUrl": "/cdn/shenron/games/budokai-tenkaichi-3-eu.rvz",
                "emulatorUrl": "/emulator/index.html?game=/cdn/shenron/games/budokai-tenkaichi-3-eu.rvz&autoboot=1",
                "minigameUrl": "/shenron",
                "reInspectUrl": "/v1/re/inspect/budokai-tenkaichi-3-eu",
                "rangeSupport": "bytes (HTTP 206 Partial Content)"
            }
        ],
        "audio_tracks": audio_tracks,
        "proof_frames_base": "/cdn/shenron/frames/",
        "textures_base": "/cdn/shenron/textures/",
        "packages": packages
    }))
    .into_response()
}

