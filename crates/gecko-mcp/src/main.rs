use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use anyhow::{Context, Result};
use backend_wgpu::GxRenderer;
use clap::Parser;
use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

use mcp::server::McpServer;
use mcp::sink::Introspection;
use mcp::state::{EmuState, RunMode, Shared};
use mcp::{boot, worker};

#[derive(Parser, Debug)]
#[command(name = "mcp", about = "MCP server exposing the gecko GameCube/Wii emulator to LLMs")]
struct Cli {
    #[arg(
        long,
        value_name = "PATH",
        help = "Disc image to load on startup (ISO/RVZ/ZIP)",
        conflicts_with = "dol"
    )]
    dvd: Option<PathBuf>,

    #[arg(long, value_name = "PATH", help = "DOL homebrew to load on startup")]
    dol: Option<PathBuf>,

    #[arg(
        long,
        help = "Boot the DOL as a Wii executable instead of GameCube",
        requires = "dol"
    )]
    wii: bool,

    #[arg(
        long,
        value_name = "PATH",
        help = "Decoded GameCube IPL to boot discs with instead of IPL HLE"
    )]
    ipl: Option<PathBuf>,

    #[arg(long, value_name = "PATH", help = "DSP IROM to use instead of Dolphin's DSP ROM")]
    dsp: Option<PathBuf>,

    #[arg(long, value_name = "PATH", help = "DSP COEF ROM to use instead of Dolphin's DSP ROM")]
    coef: Option<PathBuf>,

    #[arg(long, help = "Resume the emulator immediately after loading the disc")]
    resume: bool,

    #[arg(
        long,
        value_name = "FRAMES",
        help = "Run N frames headlessly, dump telemetry/screenshots, and exit instead of starting MCP stdio"
    )]
    smoke_frames: Option<u32>,

    #[arg(
        long,
        value_name = "DIR",
        help = "Directory to write captured PNG frames and JSON telemetry when --smoke-frames is used"
    )]
    dump_dir: Option<PathBuf>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mcp=info,gecko=info,warn")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();

    let ipl = cli
        .ipl
        .map(|p| std::fs::read(&p).with_context(|| format!("read IPL {p:?}")))
        .transpose()?;

    let dsp_rom = cli
        .dsp
        .map(|p| std::fs::read(&p).with_context(|| format!("read DSP IROM {p:?}")))
        .transpose()?;

    let coef_rom = cli
        .coef
        .map(|p| std::fs::read(&p).with_context(|| format!("read DSP COEF ROM {p:?}")))
        .transpose()?;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all().with_env(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .context("no compatible wgpu adapter")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .context("acquire wgpu device")?;
    let surface_format = wgpu::TextureFormat::Rgba8Unorm;

    let gx = Arc::new(Mutex::new(GxRenderer::new(&device, &queue, surface_format, 1)));
    let introspect = Arc::new(Mutex::new(Introspection::default()));

    let shared = Arc::new(Shared {
        state: Mutex::new(EmuState::new()),
        cv: Condvar::new(),
        gx: gx.clone(),
        device: device.clone(),
        queue: queue.clone(),
        introspect: introspect.clone(),
        ipl,
        dsp_rom,
        coef_rom,
    });

    let initial = if let Some(dol_path) = cli.dol.as_ref() {
        let bytes = std::fs::read(dol_path).with_context(|| format!("read DOL {dol_path:?}"))?;
        Some(boot::boot_dol(
            bytes,
            cli.wii,
            shared.dsp_rom.as_deref(),
            shared.coef_rom.as_deref(),
            shared.gx.clone(),
            shared.device.clone(),
            shared.queue.clone(),
            shared.introspect.clone(),
        ))
    } else if let Some(dvd_path) = cli.dvd.as_ref() {
        let bytes = std::fs::read(dvd_path).with_context(|| format!("read DVD {dvd_path:?}"))?;
        Some(boot::boot(
            bytes,
            shared.ipl.as_deref(),
            shared.dsp_rom.as_deref(),
            shared.coef_rom.as_deref(),
            shared.gx.clone(),
            shared.device.clone(),
            shared.queue.clone(),
            shared.introspect.clone(),
        ))
    } else {
        None
    };

    if let Some(result) = initial {
        let mut s = shared.state.lock().unwrap();
        s.backend = Some(result.backend);
        s.game_name = result.game_name;
        s.game_code = result.game_code;
        s.run_mode = if cli.resume { RunMode::Running } else { RunMode::Paused };
        drop(s);

        shared.cv.notify_all();
    }

    if let Some(total_frames) = cli.smoke_frames {
        let dump_dir = cli
            .dump_dir
            .unwrap_or_else(|| PathBuf::from("userland/build/re/budokai-tenkaichi-3-eu/frames"));
        std::fs::create_dir_all(&dump_dir)?;
        let mut s = shared.state.lock().unwrap();
        let game_name = s.game_name.clone();
        let game_code = s.game_code.clone();
        let backend = s.backend.as_mut().context("no game loaded for --smoke-frames")?;
        let is_wii = matches!(backend, mcp::state::Backend::Wii(_));
        tracing::info!(
            game_name = %game_name,
            game_code = %game_code,
            is_wii,
            total_frames,
            "Starting headless WebGPU smoke run"
        );
        let mut telemetry = Vec::new();
        for f in 1..=total_frames {
            // Simulate pressing Left then A + Plus periodically to advance strap/controller/save/title/menu screens
            // (In horizontal Yes/No prompts, Left moves cursor from default "No" to "Yes" before pressing A)
            if f >= 45 {
                let phase = f % 24;
                if (2080..=2350).contains(&f) && phase < 5 {
                    backend.apply_host_input(&gecko::HostInput::Wii {
                        nunchuk_attached: true,
                        sideways: false,
                        wiimote_buttons: 0x0001, // BTN_LEFT (for Yes/No save prompt only)
                        wiimote_shake: false,
                        nunchuk_buttons: 0x00,
                        nunchuk_stick_x: 0x10,
                        nunchuk_stick_y: 0x80,
                        ir_pointer: Some((260, 470)),
                        accel: None,
                    });
                } else if phase >= 10 && phase < 16 {
                    backend.apply_host_input(&gecko::HostInput::Wii {
                        nunchuk_attached: true,
                        sideways: false,
                        wiimote_buttons: 0x0018, // BTN_A (0x0008) | BTN_PLUS (0x0010)
                        wiimote_shake: false,
                        nunchuk_buttons: 0x02,   // C button
                        nunchuk_stick_x: 0x80,
                        nunchuk_stick_y: 0x80,
                        ir_pointer: Some((260, 470)),
                        accel: None,
                    });
                } else {
                    backend.apply_host_input(&gecko::HostInput::wii_neutral());
                }
            }
            backend.run_until_vsync();
            if f == 60 {
                if let mcp::state::Backend::Wii(sys) = backend {
                    let r13 = sys.gekko.gprs[13];
                    tracing::info!(
                        r13 = format!("{:#010X}", r13),
                        resetting_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x6338))),
                        enabled_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x6324))),
                        recalibrate_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x632C))),
                        waiting_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x6330))),
                        checking_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x6334))),
                        pending_bits = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x6328))),
                        resetting_chan = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x7E64))),
                        pad_spec = format!("{:#010X}", sys.mmio.virt_read_u32(r13.wrapping_sub(0x7E58))),
                        si_chan = format!("{:#010X}", sys.mmio.virt_read_u32(0x80392930)),
                        si_poll = format!("{:#010X}", sys.mmio.virt_read_u32(0x80392934)),
                        si_type0 = format!("{:#010X}", sys.mmio.virt_read_u32(0x80392948)),
                        hw_poll = format!("{:#010X}", sys.si.poll.raw()),
                        hw_comcsr = format!("{:#010X}", sys.si.comcsr.raw()),
                        hw_status = format!("{:#010X}", sys.si.status.raw()),
                        hw_in_hi0 = format!("{:#010X}", sys.si.channels[0].in_hi),
                        "PAD & SI State at frame 60"
                    );
                }
            }
            if f == 1 || f % 30 == 0 || f == total_frames {
                let (frame_idx, xfb_size, tex_count, draw_count) = {
                    let intro = shared.introspect.lock().unwrap();
                    (
                        intro.frame_index,
                        intro.last_xfb_size,
                        intro.textures.len(),
                        intro.draw_count_this_frame,
                    )
                };
                tracing::info!(
                    vsync = f,
                    pc = format!("{:#010X}", backend.pc()),
                    cycles = backend.cycles(),
                    xfb_frames = frame_idx,
                    xfb_w = xfb_size.0,
                    xfb_h = xfb_size.1,
                    textures = tex_count,
                    draws = draw_count,
                    "Frame checkpoint"
                );
                let _ = shared.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
                let captured = {
                    let gx_guard = shared.gx.lock().unwrap();
                    backend_wgpu::capture::capture_texture(&shared.device, &shared.queue, &gx_guard.xfb_texture)
                        .map(|c| c.with_aspect(backend_wgpu::sink::TargetAspect::auto(is_wii)))
                };
                let mut saved_png = None;
                let mut non_black_pixels = 0usize;
                if let Some(mut cap) = captured {
                    for px in cap.rgba.chunks_exact_mut(4) {
                        if px[0] > 4 || px[1] > 4 || px[2] > 4 {
                            non_black_pixels += 1;
                        }
                        px[3] = 255;
                    }
                    let png_path = dump_dir.join(format!("frame_{f:04}.png"));
                    let mut buf = Vec::with_capacity(cap.rgba.len());
                    {
                        let mut enc = png::Encoder::new(&mut buf, cap.width, cap.height);
                        enc.set_color(png::ColorType::Rgba);
                        enc.set_depth(png::BitDepth::Eight);
                        if let Ok(mut w) = enc.write_header() {
                            let _ = w.write_image_data(&cap.rgba);
                        }
                    }
                    if std::fs::write(&png_path, &buf).is_ok() {
                        saved_png = Some(png_path.display().to_string());
                    }
                }
                telemetry.push(serde_json::json!({
                    "vsync": f,
                    "pc": format!("{:#010X}", backend.pc()),
                    "cycles": backend.cycles(),
                    "xfb_frames": frame_idx,
                    "xfb_width": xfb_size.0,
                    "xfb_height": xfb_size.1,
                    "textures_uploaded": tex_count,
                    "non_black_pixels": non_black_pixels,
                    "png_path": saved_png,
                }));
            }
        }
        let report_path = dump_dir.join("smoke_telemetry.json");
        std::fs::write(
            &report_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "game_name": game_name,
                "game_code": game_code,
                "total_vsyncs": total_frames,
                "final_pc": format!("{:#010X}", backend.pc()),
                "final_cycles": backend.cycles(),
                "checkpoints": telemetry,
            }))?,
        )?;
        println!("Wrote smoke telemetry to {}", report_path.display());
        return Ok(());
    }

    let worker_shared = shared.clone();
    std::thread::Builder::new()
        .name("mcp-emu".into())
        .spawn(move || worker::run(worker_shared))
        .context("spawn emu worker")?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    runtime.block_on(async move {
        let server = McpServer::new(shared);
        let service = server.serve(stdio()).await.context("rmcp serve")?;
        service.waiting().await.context("rmcp waiting")?;
        anyhow::Ok(())
    })?;

    Ok(())
}
