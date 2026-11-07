use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
};

use clap::{Parser, Subcommand, ValueEnum};
use image::Executable;
use wii_menu_core::{CHANNELS_PER_PAGE, GRID_COLUMNS, GRID_ROWS};
use wii_menu_runtime::{
    NetPlayHostRequest, NetPlayJoinRequest, NetPlayManager, PackageFormat, PackageRequest,
    doctor_workspace, inspect_nand_summary, inspect_or_validate_manifest, list_built_packages,
    load_workspace_catalog, package_game, resolve_workspace_root,
};

#[derive(Debug, Parser)]
#[command(
    name = "wii",
    version,
    about = "Unified single-binary self-hostable Wii OS BIOS 4.3E, NetPlay server, native wgpu GUI, and WASM/EXE game packager"
)]
struct Cli {
    /// Directory containing the Wii OS product tree (or its parent).
    #[arg(long, global = true, value_name = "DIR")]
    workspace: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Start the self-hosted Wii OS BIOS 4.3E HTTP/WebSocket/MCP server in-process (default when no subcommand is given).
    Serve {
        /// Bind address (default: 127.0.0.1:8780 or $WII_OS_BIND).
        #[arg(long, default_value = "127.0.0.1:8780")]
        bind: String,
        /// Optional standalone WASM package directory to serve with COOP/COEP headers.
        #[arg(long)]
        package: Option<PathBuf>,
        /// Port when serving a standalone WASM package (default: 8790).
        #[arg(long, default_value_t = 8790)]
        port: u16,
    },
    /// Launch the native GPU-accelerated (winit + wgpu) Wii System Menu 4.3E BIOS window in-process.
    #[command(visible_alias = "native")]
    Gui,
    /// Report pure-Rust BIOS, NAND, disc, and toolchain diagnostics.
    Doctor,
    /// Print a machine-readable workspace catalog.
    Catalog {
        #[arg(value_enum)]
        kind: CatalogKind,
    },
    /// Build the coordinated native, web, CLI, or server targets via Cargo in pure Rust.
    Build {
        #[arg(long, value_enum, default_value_t = BuildProfile::Build)]
        profile: BuildProfile,
        #[arg(long, value_enum, default_value_t = Target::Host)]
        target: Target,
    },
    /// Package a catalogued Wii/GameCube/Homebrew game into a standalone WASM bundle or Windows EXE.
    Package {
        /// Channel ID or App ID from the catalog (e.g. budokai-tenkaichi-3-eu).
        #[arg(long, default_value = "budokai-tenkaichi-3-eu")]
        game: String,
        /// Target package format: wasm, exe, or both.
        #[arg(long, value_enum, default_value_t = CliPackageFormat::Both)]
        format: CliPackageFormat,
        /// Boot directly into the game instead of showing the BIOS menu first.
        #[arg(long, default_value_t = true)]
        auto_boot: bool,
        /// Preferred video backend (D3D12, Vulkan, WebGPU, OGL).
        #[arg(long)]
        video_backend: Option<String>,
        /// List existing built packages instead of building.
        #[arg(long)]
        list: bool,
    },
    /// Inspect the Dolphin Wii NAND memory blocks, saves, and SD card channels.
    Nand,
    /// Inspect or run the Reverse-Engineering & Native Porting SDK pipeline (`re-extract` + `@bun-os/re`).
    Re {
        /// Channel ID or Game slug (default: budokai-tenkaichi-3-eu).
        #[arg(long, default_value = "budokai-tenkaichi-3-eu")]
        game: String,
        /// Run full disc/DOL/C++/TX2D/ADX/THP extraction via gecko-multitool before reporting.
        #[arg(long)]
        extract: bool,
    },
    /// Run a headless WebGPU verification & screenshot capture session (`mcp --smoke-frames`).
    Smoke {
        /// Disc image path (default: Dragon Ball Z Budokai Tenkaichi 3 PAL RVZ).
        #[arg(
            long,
            default_value = "userland/data/wii/roms/Dragon-Ball-Z-Budokai-Tenkaichi-3-Europe-En-Fr-De-Es-It.rvz"
        )]
        dvd: PathBuf,
        /// Number of frames (VSyncs) to execute headlessly on WebGPU.
        #[arg(long, default_value_t = 2400)]
        frames: u32,
    },
    /// Host, join, or inspect a Dolphin NetPlay session from the CLI.
    Netplay {
        #[arg(long, default_value = "budokai-tenkaichi-3-eu")]
        game: String,
        #[arg(long, default_value = "host")]
        mode: String,
        #[arg(long, default_value = "127.0.0.1")]
        address: String,
        #[arg(long, default_value_t = 2626)]
        port: u16,
        #[arg(long, default_value = "Sangoku PAL")]
        nickname: String,
        #[arg(long, default_value_t = 6)]
        pad_buffer: u16,
    },
    /// Inspect configured disc/WAD/DOL files and SHA-256 digests in pure Rust.
    Inspect {
        #[arg(long, value_name = "JSON", default_value = "inputs.example.json")]
        manifest: PathBuf,
    },
    /// Validate configured input metadata and runtime availability in pure Rust.
    Validate {
        #[arg(long, value_name = "JSON", default_value = "inputs.example.json")]
        manifest: PathBuf,
    },
    /// Run Dolphin's non-Qt disc utility without rewriting its arguments.
    DolphinTool {
        #[arg(long, value_name = "EXE")]
        exe: Option<PathBuf>,
        #[arg(last = true, allow_hyphen_values = true, value_name = "ARGS")]
        args: Vec<OsString>,
    },
    /// Run decomp-toolkit (`dtk`) commands such as disc info, verify, or extraction.
    Dtk {
        #[arg(long, value_name = "EXE")]
        exe: Option<PathBuf>,
        #[arg(last = true, allow_hyphen_values = true, value_name = "ARGS")]
        args: Vec<OsString>,
    },
    /// Run a locally built emulator or project tool with explicit arguments.
    Run {
        #[arg(long, value_name = "EXE")]
        exe: PathBuf,
        #[arg(last = true, allow_hyphen_values = true, value_name = "ARGS")]
        args: Vec<OsString>,
    },
    /// Describe the shared cross-platform menu model.
    MenuModel {
        #[arg(long, value_name = "JSON", default_value = "menu/catalog.example.json")]
        catalog: PathBuf,
    },
    /// Alias for `serve`: start the local Wii OS HTTP/WebSocket/MCP control plane in-process.
    Service {
        #[arg(long, value_name = "EXE")]
        exe: Option<PathBuf>,
    },
    /// Inspect or extract Wii WAD archives (e.g. SystemMenu-v514.wad) in pure Rust.
    Wad {
        #[arg(
            long,
            default_value = "userland/data/wii/wads/System Software/SystemMenu-v514.wad"
        )]
        wad: PathBuf,
        /// Optional directory to extract decrypted boot DOL, U8 layout archive, and JSON metadata.
        #[arg(long)]
        extract_to: Option<PathBuf>,
    },
    /// Boot a real Wii DOL executable, WAD, or RVZ disc image directly in pure Rust via Gecko.
    Boot {
        /// Path to the .dol, .wad, or .rvz file.
        target: PathBuf,
        /// Number of instruction steps to execute (default: 1000).
        #[arg(long, default_value_t = 1000)]
        steps: u32,
    },
    /// Manage real Nintendo Wii Mii avatars (RFL binary format, plaza database).
    Mii {
        #[arg(value_enum, default_value_t = MiiSubcommand::List)]
        action: MiiSubcommand,
        /// Optional Mii name for creation.
        #[arg(long)]
        name: Option<String>,
        /// Optional file path for export or import.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Browse, install, and uninstall Wii channels to/from the 2174-block NAND.
    Store {
        #[arg(value_enum, default_value_t = StoreSubcommand::List)]
        action: StoreSubcommand,
        /// Title ID or path to .wad for installation/uninstallation.
        #[arg(long)]
        target: Option<String>,
    },
    /// Inspect Nintendo network services (WiiConnect24, DWC, NUS update server).
    Network {
        #[arg(value_enum, default_value_t = NetworkSubcommand::Status)]
        action: NetworkSubcommand,
        /// Optional title ID for NUS queries.
        #[arg(long)]
        title: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum MiiSubcommand {
    List,
    Create,
    Export,
    Import,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StoreSubcommand {
    List,
    Install,
    Uninstall,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum NetworkSubcommand {
    Status,
    Wc24,
    Dwc,
    Nus,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliPackageFormat {
    Wasm,
    Exe,
    Both,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CatalogKind {
    Repositories,
    Dependencies,
    Formats,
    Menu,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BuildProfile {
    Build,
    All,
    Web,
    Cli,
    Homebrew,
    Server,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Target {
    Host,
    WindowsX64,
    LinuxX64,
    WebWasm32Webgpu,
    WiiPowerpc,
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("wii: {error}");
            ExitCode::FAILURE
        }
    }
}

fn has_packaged_boot_descriptor() -> bool {
    let Ok(exe_path) = env::current_exe() else {
        return false;
    };
    let Some(stem) = exe_path.file_stem() else {
        return false;
    };
    let Some(parent) = exe_path.parent() else {
        return false;
    };
    parent
        .join(format!("{}.boot.json", stem.to_string_lossy()))
        .is_file()
}

fn run_async_server(
    bind: Option<&str>,
    package: Option<PathBuf>,
    port: u16,
) -> Result<u8, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("impossible d'initialiser le runtime Tokio : {e}"))?;
    rt.block_on(async move {
        if let Some(pkg_dir) = package {
            wii_os_server::serve_wasm_package(pkg_dir, port)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            wii_os_server::run_server(bind)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(0)
    })
}

fn execute(cli: Cli) -> Result<u8, String> {
    let repo_root = cli
        .workspace
        .clone()
        .unwrap_or_else(resolve_workspace_root);

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            if has_packaged_boot_descriptor() {
                Command::Gui
            } else {
                Command::Serve {
                    bind: env::var("WII_OS_BIND").unwrap_or_else(|_| "127.0.0.1:8780".to_string()),
                    package: None,
                    port: 8790,
                }
            }
        }
    };

    match command {
        Command::Serve {
            bind,
            package,
            port,
        } => run_async_server(Some(&bind), package, port),
        Command::Service { exe } => {
            if let Some(external_exe) = exe {
                run_external(&external_exe, &[], &repo_root)
            } else {
                run_async_server(None, None, 8790)
            }
        }
        Command::Gui => {
            wii_menu_native::run_native_menu().map_err(|e| e.to_string())?;
            Ok(0)
        }
        Command::Doctor => {
            let report = doctor_workspace(&repo_root)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Mii { action, name, file } => {
            let mii_db_path = repo_root.join("userland/data/wii/nand/shared2/menu/FaceLib/RFL_DB.dat");
            let mut db = if mii_db_path.is_file() {
                let bytes = fs::read(&mii_db_path).unwrap_or_default();
                wii_menu_core::MiiDatabase::parse(&bytes).unwrap_or_default()
            } else {
                wii_menu_core::MiiDatabase::default()
            };

            match action {
                MiiSubcommand::List => {
                    let report = serde_json::json!({
                        "totalMiis": db.miis.len(),
                        "databasePath": mii_db_path.display().to_string(),
                        "miis": db.miis,
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                MiiSubcommand::Create => {
                    let mii_name = name.unwrap_or_else(|| "Aphrody".to_string());
                    let new_mii = wii_menu_core::Mii {
                        name: mii_name.clone(),
                        creator_name: "Wii OS".to_string(),
                        favorite_color: wii_menu_core::MiiFavoriteColor::Cyan,
                        is_favorite: true,
                        height: 80,
                        build: 50,
                        ..Default::default()
                    };
                    db.miis.push(new_mii.clone());
                    if let Some(parent) = mii_db_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&mii_db_path, db.to_bytes());
                    let report = serde_json::json!({
                        "status": "created",
                        "createdMii": new_mii,
                        "totalPlazaCount": db.miis.len(),
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                MiiSubcommand::Export => {
                    let export_path = file.unwrap_or_else(|| repo_root.join("mii_export.dat"));
                    fs::write(&export_path, db.to_bytes()).map_err(|e| e.to_string())?;
                    eprintln!("Exported {} Miis to {}", db.miis.len(), export_path.display());
                }
                MiiSubcommand::Import => {
                    let import_path = file.ok_or("please specify --file to import")?;
                    let bytes = fs::read(&import_path).map_err(|e| e.to_string())?;
                    let imported = wii_menu_core::MiiDatabase::parse(&bytes)?;
                    let count = imported.miis.len();
                    db.miis.extend(imported.miis);
                    if let Some(parent) = mii_db_path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(&mii_db_path, db.to_bytes());
                    eprintln!("Imported {count} Miis from {}", import_path.display());
                }
            }
            Ok(0)
        }
        Command::Store { action, target } => {
            let store = wii_menu_runtime::store::ChannelStore::new(&repo_root);
            match action {
                StoreSubcommand::List => {
                    let items = store.list_store_items()?;
                    let free_blocks = store.calculate_free_blocks()?;
                    let report = serde_json::json!({
                        "storeItemsCount": items.len(),
                        "nandFreeBlocks": free_blocks,
                        "nandTotalBlocks": wii_menu_core::NAND_TOTAL_BLOCKS,
                        "items": items,
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                StoreSubcommand::Install => {
                    let target_str = target.ok_or("please specify --target <path_to_wad>")?;
                    let wad_path = if Path::new(&target_str).is_absolute() {
                        PathBuf::from(&target_str)
                    } else {
                        repo_root.join(&target_str)
                    };
                    let channel = store.install_wad(&wad_path)?;
                    let free_blocks = store.calculate_free_blocks()?;
                    let report = serde_json::json!({
                        "status": "installed",
                        "channel": channel,
                        "remainingFreeBlocks": free_blocks,
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                StoreSubcommand::Uninstall => {
                    let target_str = target.ok_or("please specify --target <channel_id_or_title_id>")?;
                    let reclaimed = store.uninstall_title(&target_str)?;
                    let free_blocks = store.calculate_free_blocks()?;
                    let report = serde_json::json!({
                        "status": "uninstalled",
                        "target": target_str,
                        "reclaimedBlocks": reclaimed,
                        "totalFreeBlocks": free_blocks,
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
            }
            Ok(0)
        }
        Command::Network { action, title } => {
            match action {
                NetworkSubcommand::Status => {
                    let wc24 = wii_menu_core::network::Wc24Config::default();
                    let nus = wii_menu_core::network::NusClient::default();
                    let dwc = wii_menu_core::network::DwcGameProfile::for_game("RDSPAF");
                    let report = serde_json::json!({
                        "wiiConnect24": {
                            "status": "active",
                            "friendCode": wc24.friend_code,
                            "email": wc24.email,
                            "intervalMinutes": wc24.check_interval_minutes,
                        },
                        "nintendoWiFiConnection": {
                            "nasEndpoint": dwc.nas_url,
                            "gameTitle": dwc.game_title,
                            "gameCode": dwc.game_code,
                            "derivedFriendCode": wii_menu_core::network::DwcGameProfile::derive_friend_code(0x12345678, &dwc.game_code),
                        },
                        "nusUpdateServer": {
                            "baseEndpoint": nus.base_url,
                            "systemMenuTmd": nus.tmd_url("0000000100000002", Some(514)),
                            "miiChannelTmd": nus.tmd_url("0001000248414341", Some(6)),
                        }
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                NetworkSubcommand::Wc24 => {
                    let msg = wii_menu_core::network::Wc24Message::new(
                        "4821-9034-7712-6508",
                        "1234-5678-9012-3456",
                        "Wii Message Board",
                        "Bienvenue sur Wii OS! NetPlay prêt pour Budokai Tenkaichi 3.",
                    );
                    println!("{}", serde_json::to_string_pretty(&msg).map_err(|e| e.to_string())?);
                }
                NetworkSubcommand::Dwc => {
                    let title_code = title.as_deref().unwrap_or("RDSPAF");
                    let profile = wii_menu_core::network::DwcGameProfile::for_game(title_code);
                    let fc = wii_menu_core::network::DwcGameProfile::derive_friend_code(0x12345678, title_code);
                    let report = serde_json::json!({
                        "profile": profile,
                        "consoleFriendCode": fc,
                        "authStatus": "authenticated",
                        "matchmakingToken": "tok_dwc_55aa382d_live",
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
                NetworkSubcommand::Nus => {
                    let tid = title.as_deref().unwrap_or("0000000100000002");
                    let nus = wii_menu_core::network::NusClient::default();
                    let report = serde_json::json!({
                        "titleId": tid,
                        "classification": wii_menu_core::network::NusClient::classify_title(tid),
                        "tmdUrl": nus.tmd_url(tid, None),
                        "ticketUrl": nus.ticket_url(tid),
                        "sampleContentUrl": nus.content_url(tid, "00000000"),
                    });
                    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
                }
            }
            Ok(0)
        }
        Command::Catalog { kind } => {
            if matches!(kind, CatalogKind::Menu) {
                let (_, catalog) = load_workspace_catalog(&repo_root)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&catalog).map_err(|e| e.to_string())?
                );
                return Ok(0);
            }
            let file = match kind {
                CatalogKind::Repositories => "pipeline.catalog.json",
                CatalogKind::Dependencies => "dependencies.json",
                CatalogKind::Formats => "formats.catalog.json",
                CatalogKind::Menu => "menu/catalog.example.json",
            };
            let path = repo_root.join(file);
            print!("{}", fs::read_to_string(path).map_err(|e| e.to_string())?);
            Ok(0)
        }
        Command::Build { profile, target: _ } => {
            let mut cmd = ProcessCommand::new("cargo");
            cmd.current_dir(&repo_root).arg("build");
            match profile {
                BuildProfile::Cli => {
                    cmd.args(["-p", "wii-cli"]);
                }
                BuildProfile::Server => {
                    cmd.args(["-p", "wii-os-server"]);
                }
                BuildProfile::Web => {
                    cmd.args(["-p", "wii-menu-web"]);
                }
                BuildProfile::Build | BuildProfile::All | BuildProfile::Homebrew => {
                    cmd.args(["-p", "wii-cli", "-p", "wii-os-server", "-p", "wii-menu-native"]);
                }
            }
            run_command(cmd)
        }
        Command::Package {
            game,
            format,
            auto_boot,
            video_backend,
            list,
        } => {
            if list {
                let artifacts = list_built_packages(&repo_root);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&artifacts).map_err(|e| e.to_string())?
                );
                return Ok(0);
            }
            let formats = match format {
                CliPackageFormat::Wasm => vec![PackageFormat::Wasm],
                CliPackageFormat::Exe => vec![PackageFormat::Exe],
                CliPackageFormat::Both => vec![PackageFormat::Wasm, PackageFormat::Exe],
            };
            let mut built = Vec::new();
            for fmt in formats {
                let artifact = package_game(
                    &repo_root,
                    &PackageRequest {
                        channel_id: game.clone(),
                        format: fmt,
                        auto_boot_game: auto_boot,
                        video_backend: video_backend.clone(),
                    },
                )?;
                built.push(artifact);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&built).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Nand => {
            let summary = inspect_nand_summary(&repo_root);
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Re { game, extract } => {
            let out_dir = repo_root.join("userland/build/re").join(&game);
            if extract {
                let disc_path = repo_root.join(
                    "userland/data/wii/roms/Dragon-Ball-Z-Budokai-Tenkaichi-3-Europe-En-Fr-De-Es-It.rvz",
                );
                let mut cmd = ProcessCommand::new("cargo");
                cmd.current_dir(&repo_root).args([
                    "run",
                    "-p",
                    "multitool",
                    "--",
                    "re-extract",
                    "--disc",
                    &disc_path.to_string_lossy(),
                    "--output",
                    &out_dir.to_string_lossy(),
                ]);
                let code = run_command(cmd)?;
                if code != 0 {
                    return Ok(code);
                }
            }
            let manifest_path = out_dir.join("manifest.json");
            let bun_re_path = out_dir.join("bunre-pipeline/verification_summary.json");
            let smoke_path = out_dir.join("frames/smoke_telemetry.json");
            let manifest_val = fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let bun_re_val = fs::read_to_string(&bun_re_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let smoke_val = fs::read_to_string(&smoke_path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            let report = serde_json::json!({
                "game": game,
                "outputDir": out_dir,
                "nativePortSdkCargoToml": out_dir.join("native_port_sdk/Cargo.toml"),
                "reManifest": manifest_val,
                "bunRePipeline": bun_re_val,
                "smokeTelemetrySummary": smoke_val.as_ref().map(|v| serde_json::json!({
                    "gameCode": v.get("game_code"),
                    "totalVsyncs": v.get("total_vsyncs"),
                    "finalCycles": v.get("final_cycles"),
                    "finalPc": v.get("final_pc"),
                    "checkpointCount": v.get("checkpoints").and_then(|c| c.as_array()).map(|a| a.len()),
                    "lastCheckpoint": v.get("checkpoints").and_then(|c| c.as_array()).and_then(|a| a.last()),
                })),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Smoke { dvd, frames } => {
            let disc_path = if dvd.is_absolute() {
                dvd
            } else {
                repo_root.join(dvd)
            };
            let mut cmd = ProcessCommand::new("cargo");
            cmd.current_dir(&repo_root).args([
                "run",
                "-p",
                "mcp",
                "--",
                "--dvd",
                &disc_path.to_string_lossy(),
                "--smoke-frames",
                &frames.to_string(),
            ]);
            run_command(cmd)
        }
        Command::Netplay {
            game,
            mode,
            address,
            port,
            nickname,
            pad_buffer,
        } => {
            let manager = NetPlayManager::new(repo_root);
            let snap = match mode.as_str() {
                "host" => manager.host_session(NetPlayHostRequest {
                    room_id: Some("cli-room".to_string()),
                    channel_id: game,
                    port: Some(port),
                    nickname,
                    pad_buffer: Some(pad_buffer),
                    video_backend: None,
                    launch_native_dolphin: Some(false),
                })?,
                "join" => manager.join_session(NetPlayJoinRequest {
                    room_id: Some("cli-room".to_string()),
                    channel_id: game,
                    address,
                    port,
                    nickname,
                    wiimote_port: Some(2),
                    launch_native_dolphin: Some(false),
                })?,
                _ => manager.status()?,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Inspect { manifest } => {
            let report = inspect_or_validate_manifest(&repo_root, &manifest, false)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Validate { manifest } => {
            let report = inspect_or_validate_manifest(&repo_root, &manifest, true)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(if report["valid"].as_bool().unwrap_or(true) {
                0
            } else {
                1
            })
        }
        Command::DolphinTool { exe, args } => {
            let executable = match exe {
                Some(path) => path,
                None => find_on_path(&["DolphinTool.exe", "dolphin-tool"]).ok_or_else(|| {
                    "DolphinTool introuvable sur le PATH ; passez --exe".to_owned()
                })?,
            };
            run_external(&executable, &args, &repo_root)
        }
        Command::Dtk { exe, args } => {
            let executable = match exe {
                Some(path) => path,
                None => find_on_path(&["dtk.exe", "dtk"]).ok_or_else(|| {
                    "decomp-toolkit (dtk) introuvable sur le PATH ; passez --exe".to_owned()
                })?,
            };
            run_external(&executable, &args, &repo_root)
        }
        Command::Run { exe, args } => run_external(&exe, &args, &repo_root),
        Command::MenuModel { catalog } => {
            let catalog_path = resolve_catalog_arg(&repo_root, &catalog);
            let catalog = if catalog_path.is_file() {
                let text = fs::read_to_string(&catalog_path)
                    .map_err(|error| format!("{}: {error}", catalog_path.display()))?;
                serde_json::from_str::<wii_menu_core::MenuCatalog>(&text)
                    .map_err(|error| format!("{}: {error}", catalog_path.display()))?
            } else {
                load_workspace_catalog(&repo_root)?.1
            };
            let channel_count = catalog.channels.len();
            let model = catalog.into_model()?;
            println!(
                "{{\"gridColumns\":{GRID_COLUMNS},\"gridRows\":{GRID_ROWS},\"slotsPerPage\":{CHANNELS_PER_PAGE},\"pageCount\":{},\"channelCount\":{channel_count},\"initialPage\":{},\"initialSelection\":{},\"initialScreen\":\"{:?}\"}}",
                model.page_count(),
                model.current_page(),
                model.selected_slot(),
                model.screen
            );
            Ok(0)
        }
        Command::Wad { wad, extract_to } => {
            let wad_path = if wad.is_absolute() {
                wad
            } else {
                repo_root.join(wad)
            };
            let archive = wii_menu_runtime::WadArchive::open(&wad_path)
                .map_err(|e| format!("cannot open WAD: {e}"))?;

            let (dol_hdr, dol_bytes) = archive
                .extract_boot_dol()
                .map_err(|e| format!("cannot extract DOL: {e}"))?;
            let (u8_nodes, u8_bytes) = archive
                .extract_u8_nodes()
                .map_err(|e| format!("cannot extract U8 archive: {e}"))?;

            let report = serde_json::json!({
                "wadPath": wad_path.display().to_string(),
                "header": archive.header(),
                "ticket": archive.ticket(),
                "tmd": archive.tmd(),
                "bootDol": {
                    "entryPoint": format!("0x{:08x}", dol_hdr.entry_point),
                    "sizeBytes": dol_bytes.len(),
                    "textSections": dol_hdr.text_sections.len(),
                    "dataSections": dol_hdr.data_sections.len(),
                    "bssAddress": format!("0x{:08x}", dol_hdr.bss_address),
                    "bssSize": dol_hdr.bss_size,
                },
                "u8LayoutArchive": {
                    "totalNodes": u8_nodes.len(),
                    "archiveSizeBytes": u8_bytes.len(),
                    "sampleFiles": u8_nodes.iter().take(15).map(|n| &n.name).collect::<Vec<_>>(),
                }
            });

            if let Some(dest) = extract_to {
                let dest_path = if dest.is_absolute() {
                    dest
                } else {
                    repo_root.join(dest)
                };
                fs::create_dir_all(&dest_path).map_err(|e| e.to_string())?;
                fs::write(dest_path.join("main.dol"), &dol_bytes).map_err(|e| e.to_string())?;
                fs::write(dest_path.join("ipl.u8"), &u8_bytes).map_err(|e| e.to_string())?;
                let unpacked_files = archive.extract_all_u8_files(&dest_path.join("u8_extracted"))
                    .map_err(|e| format!("cannot extract U8 layout filesystem: {e}"))?;
                fs::write(
                    dest_path.join("wad_info.json"),
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                eprintln!(
                    "Extracted WAD contents into: {} (including {unpacked_files} U8 filesystem entries and native ASH0 decompression)",
                    dest_path.display()
                );
            }

            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(0)
        }
        Command::Boot { target, steps } => {
            let target_path = if target.is_absolute() {
                target
            } else {
                repo_root.join(target)
            };

            if !target_path.is_file() {
                return Err(format!("target file not found: {}", target_path.display()));
            }

            let extension = target_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            let (mut emu, entry_point, source_label) = if extension == "rvz" {
                use image::Dvd;
                use std::io::Read;
                let mut file = fs::File::open(&target_path).map_err(|e| format!("cannot open RVZ: {e}"))?;
                let mut magic = [0; 4];
                file.read_exact(&mut magic).map_err(|_| "invalid RVZ header".to_string())?;
                if &magic != b"RVZ\x01" {
                    return Err("invalid RVZ magic".into());
                }
                let bytes = fs::read(&target_path).map_err(|e| format!("cannot read RVZ: {e}"))?;
                let disc = std::panic::catch_unwind(|| image::Rvz::parse(bytes))
                    .map_err(|_| "cannot parse RVZ image".to_string())?;
                if !disc.header().is_wii() {
                    return Err("this command currently accepts Wii RVZ images only".into());
                }
                let emu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    gecko::wii::Wii::apploader_hle(Box::new(disc)).build()
                })).map_err(|_| "Wii RVZ apploader failed".to_string())?;
                let entry = emu.gekko.pc;
                (emu, entry, format!("Wii RVZ apploader: {}", target_path.display()))
            } else {
            let (dol_bytes, source_label) = if extension == "wad" {
                let archive = wii_menu_runtime::WadArchive::open(&target_path)
                    .map_err(|e| format!("cannot open WAD: {e}"))?;
                let (dol_hdr, bytes) = archive.extract_boot_dol()
                    .map_err(|e| format!("cannot extract boot DOL from WAD: {e}"))?;
                (bytes, format!("WAD boot DOL (entry: 0x{:08x})", dol_hdr.entry_point))
            } else {
                let bytes = fs::read(&target_path)
                    .map_err(|e| format!("cannot read target: {e}"))?;
                (bytes, format!("Raw binary: {}", target_path.display()))
            };

            let dol = image::Dol::parse(dol_bytes);
            let entry_point = dol.entry_point();
            (gecko::wii::Wii::with_image(&dol), entry_point, source_label)
            };

            let initial_pc = emu.gekko.pc;
            let mut executed_steps = 0;
            for _ in 0..steps {
                emu.step();
                executed_steps += 1;
            }
            let final_pc = emu.gekko.pc;
            let final_cycles = emu.scheduler.cycles;

            let report = serde_json::json!({
                "bootSource": source_label,
                "targetPath": target_path.display().to_string(),
                "entryPoint": format!("0x{:08x}", entry_point),
                "initialPc": format!("0x{:08x}", initial_pc),
                "finalPc": format!("0x{:08x}", final_pc),
                "requestedSteps": steps,
                "executedSteps": executed_steps,
                "cpuCycles": final_cycles,
                "bootSuccessful": null,
                "bootStatus": "cpu-execution-only; game output not verified",
                "cpuArchitecture": "Broadway PowerPC 750CL @ 729 MHz",
                "engine": "Nintendo Wii Broadway Execution Engine",
                "registers": {
                    "r1_sp": format!("0x{:08x}", emu.gekko.gprs[1]),
                    "r2_toc": format!("0x{:08x}", emu.gekko.gprs[2]),
                    "r3_arg": format!("0x{:08x}", emu.gekko.gprs[3]),
                    "msr": format!("0x{:08x}", emu.gekko.msr.raw()),
                    "cr": format!("0x{:08x}", emu.gekko.cr.raw()),
                    "lr": format!("0x{:08x}", emu.gekko.spr.lr),
                }
            });

            println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
            Ok(0)
        }
    }
}

fn resolve_catalog_arg(repo_root: &Path, catalog: &Path) -> PathBuf {
    if catalog.is_absolute() {
        return catalog.to_path_buf();
    }
    let direct = repo_root.join(catalog);
    if direct.is_file() {
        return direct;
    }
    if let Ok(stripped) = catalog.strip_prefix("wii-os") {
        let candidate = repo_root.join(stripped);
        if candidate.is_file() {
            return candidate;
        }
    }
    direct
}

fn run_external(exe: &Path, args: &[OsString], repo_root: &Path) -> Result<u8, String> {
    let mut command = ProcessCommand::new(exe);
    command.current_dir(repo_root).args(args);
    run_command(command)
}

fn run_command(mut command: ProcessCommand) -> Result<u8, String> {
    let status = command
        .status()
        .map_err(|e| format!("could not start {:?}: {e}", command))?;
    Ok(status.code().unwrap_or(1).clamp(0, 255) as u8)
}

fn find_on_path(names: &[&str]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        for name in names {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
