use std::sync::Arc;

use axum::{
    Json,
    body::to_bytes,
    extract::{Path, State},
    response::IntoResponse,
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::{
        StreamableHttpServerConfig,
        streamable_http_server::{
            session::local::LocalSessionManager, tower::StreamableHttpService,
        },
    },
};
use serde::Deserialize;
use wii_menu_runtime::{NetPlayHostRequest, NetPlayJoinRequest, PackageFormat, PackageRequest};

use crate::{
    AppSettings, AppState, CreateRoom, NetPlayStartInput, build_package, create_board_message,
    create_room, db::NewBoardMessage, get_nand_summary, get_packages, get_sysconf,
    launch_app, list_apps, list_board_messages, list_rooms, netplay_host, netplay_join,
    netplay_start, netplay_status, netplay_stop,
};

#[derive(Clone)]
pub(crate) struct WiiOsMcp {
    state: AppState,
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl WiiOsMcp {
    fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
struct AppId {
    #[schemars(description = "ID d’application ou de chaîne du catalogue Wii OS")]
    app_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct VideoBackendInput {
    #[schemars(description = "Backend Dolphin : D3D12, Vulkan, OGL ou Software Renderer")]
    video_backend: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct RoomInput {
    id: String,
    title_id: Option<String>,
    max_players: Option<u32>,
    transport: Option<String>,
    channel_id: Option<String>,
    host_nickname: Option<String>,
    port: Option<u16>,
    pad_buffer: Option<u16>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct McpNetPlayHostInput {
    channel_id: String,
    nickname: String,
    port: Option<u16>,
    pad_buffer: Option<u16>,
    video_backend: Option<String>,
    launch_native_dolphin: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct McpNetPlayJoinInput {
    channel_id: String,
    address: String,
    port: u16,
    nickname: String,
    wiimote_port: Option<u8>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct McpPackageInput {
    #[schemars(description = "ID du jeu ou de la chaîne (ex: budokai-tenkaichi-3-eu)")]
    channel_id: String,
    #[schemars(description = "Format de sortie : 'wasm' ou 'exe'")]
    format: String,
    #[schemars(description = "Démarrer directement le jeu sans passer par le menu BIOS")]
    auto_boot_game: Option<bool>,
    video_backend: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct McpMemoInput {
    title: String,
    author: Option<String>,
    body: String,
}

#[tool_router]
impl WiiOsMcp {
    #[tool(
        description = "Liste les chaînes et applications du catalogue Wii OS avec leur état de contenu local."
    )]
    async fn list_apps(&self) -> String {
        let Json(apps) = list_apps(State(self.state.clone())).await;
        serde_json::to_string(&apps).unwrap_or_else(|error| format!("{{\"error\":{error}}}"))
    }

    #[tool(description = "Lit les réglages persistants du serveur Wii OS et du BIOS SYSCONF 4.3E.")]
    async fn get_settings(&self) -> CallToolResult {
        response_result(get_sysconf(State(self.state.clone())).await).await
    }

    #[tool(
        description = "Lance une chaîne ou application du catalogue via Dolphin avec son profil Wii OS isolé."
    )]
    async fn launch_app(&self, Parameters(AppId { app_id }): Parameters<AppId>) -> CallToolResult {
        response_result(launch_app(State(self.state.clone()), Path(app_id)).await).await
    }

    #[tool(description = "Modifie et persiste le backend vidéo Dolphin sélectionné.")]
    async fn set_video_backend(
        &self,
        Parameters(VideoBackendInput { video_backend }): Parameters<VideoBackendInput>,
    ) -> CallToolResult {
        response_result(
            crate::update_settings(
                State(self.state.clone()),
                Json(AppSettings {
                    video_backend: Some(video_backend),
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Inspecte la mémoire NAND Wii (blocs libres, sauvegardes Wii/GC et chaînes installées).")]
    async fn inspect_nand(&self) -> CallToolResult {
        response_result(get_nand_summary(State(self.state.clone())).await).await
    }

    #[tool(description = "Lit les messages et l'historique de jeu du Bureau Wii (Wii Message Board).")]
    async fn list_board_messages(&self) -> CallToolResult {
        response_result(list_board_messages(State(self.state.clone())).await).await
    }

    #[tool(description = "Écrit et épingle un nouveau mémo sur le Bureau Wii (Wii Message Board).")]
    async fn post_board_memo(&self, Parameters(input): Parameters<McpMemoInput>) -> CallToolResult {
        response_result(
            create_board_message(
                State(self.state.clone()),
                Json(NewBoardMessage {
                    date_key: Some("Aujourd'hui".to_string()),
                    kind: Some("memo".to_string()),
                    title: input.title,
                    author: input.author,
                    body: input.body,
                    title_id: None,
                    duration_minutes: Some(0),
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Liste les salons multijoueurs persistés dans la base SQLite.")]
    async fn list_rooms(&self) -> String {
        let Json(rooms) = list_rooms(State(self.state.clone())).await;
        serde_json::to_string(&rooms).unwrap_or_else(|error| format!("{{\"error\":{error}}}"))
    }

    #[tool(description = "Crée et persiste un salon multijoueur Dolphin NetPlay ou WebRTC dans SQLite.")]
    async fn create_room(&self, Parameters(input): Parameters<RoomInput>) -> CallToolResult {
        response_result(
            create_room(
                State(self.state.clone()),
                Json(CreateRoom {
                    id: input.id,
                    title_id: input.title_id,
                    max_players: input.max_players,
                    transport: input.transport,
                    channel_id: input.channel_id,
                    host_nickname: input.host_nickname,
                    host_address: None,
                    port: input.port,
                    pad_buffer: input.pad_buffer,
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Retourne l'état complet de la session NetPlay active (joueurs, ports Wiimote, hash du jeu, événements).")]
    async fn netplay_status(&self) -> CallToolResult {
        response_result(netplay_status(State(self.state.clone())).await).await
    }

    #[tool(description = "Héberge une session NetPlay Dolphin pour un jeu du catalogue.")]
    async fn netplay_host(
        &self,
        Parameters(input): Parameters<McpNetPlayHostInput>,
    ) -> CallToolResult {
        response_result(
            netplay_host(
                State(self.state.clone()),
                Json(NetPlayHostRequest {
                    room_id: None,
                    channel_id: input.channel_id,
                    port: input.port,
                    nickname: input.nickname,
                    pad_buffer: input.pad_buffer,
                    video_backend: input.video_backend,
                    launch_native_dolphin: input.launch_native_dolphin,
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Rejoint une session NetPlay Dolphin existante avec attribution de Télécommande Wii (P1..P4).")]
    async fn netplay_join(
        &self,
        Parameters(input): Parameters<McpNetPlayJoinInput>,
    ) -> CallToolResult {
        response_result(
            netplay_join(
                State(self.state.clone()),
                Json(NetPlayJoinRequest {
                    room_id: None,
                    channel_id: input.channel_id,
                    address: input.address,
                    port: input.port,
                    nickname: input.nickname,
                    wiimote_port: input.wiimote_port,
                    launch_native_dolphin: Some(false),
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Démarre la partie NetPlay active.")]
    async fn netplay_start(&self) -> CallToolResult {
        response_result(
            netplay_start(
                State(self.state.clone()),
                Json(NetPlayStartInput {
                    launch_dolphin_process: false,
                }),
            )
            .await,
        )
        .await
    }

    #[tool(description = "Arrête la session NetPlay active.")]
    async fn netplay_stop(&self) -> CallToolResult {
        response_result(netplay_stop(State(self.state.clone())).await).await
    }

    #[tool(description = "Liste tous les packages de jeux autonomes (WASM et EXE) déjà générés.")]
    async fn list_packages(&self) -> CallToolResult {
        response_result(get_packages(State(self.state.clone())).await).await
    }

    #[tool(description = "Package un jeu du catalogue Wii OS en bundle WebAssembly (.wasm + WebGPU) ou en exécutable Windows autonome (.exe).")]
    async fn package_game(&self, Parameters(input): Parameters<McpPackageInput>) -> CallToolResult {
        let format = if input.format.eq_ignore_ascii_case("exe") {
            PackageFormat::Exe
        } else {
            PackageFormat::Wasm
        };
        response_result(
            build_package(
                State(self.state.clone()),
                Json(PackageRequest {
                    channel_id: input.channel_id,
                    format,
                    auto_boot_game: input.auto_boot_game.unwrap_or(false),
                    video_backend: input.video_backend,
                }),
            )
            .await,
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for WiiOsMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Wii OS BIOS 4.3E, NetPlay & Packaging MCP intégré au serveur HTTP.")
    }
}

async fn response_result(response: impl IntoResponse) -> CallToolResult {
    let response = response.into_response();
    let status = response.status();
    let body = match to_bytes(response.into_body(), 1024 * 1024).await {
        Ok(body) => String::from_utf8_lossy(&body).into_owned(),
        Err(error) => format!("Impossible de lire la réponse HTTP : {error}"),
    };
    let text = format!("HTTP {}\n{}", status.as_u16(), body);
    if status.is_success() {
        CallToolResult::success(vec![ContentBlock::text(text)])
    } else {
        CallToolResult::error(vec![ContentBlock::text(text)])
    }
}

pub(crate) fn service(state: AppState) -> StreamableHttpService<WiiOsMcp, LocalSessionManager> {
    let shared_state = Arc::new(state);
    StreamableHttpService::new(
        move || Ok(WiiOsMcp::new(shared_state.as_ref().clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    )
}
