#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use egui::ViewportId;
use gecko::HostInput;
use gecko::flipper::si::pad::{self, PadStatus, STICK_CENTER, STICK_MAX, STICK_MIN, TRIGGER_MAX, TRIGGER_MIN};
use gecko::flipper::vi::regs::RefreshRate;
use gecko::gamecube::GameCube;
use gecko::hollywood::ipc::usb as wiimote;
use gecko::audio::{AidBlock, AudioSink};
use gecko::host::{DrawVertex, GxAction, RenderSink};
use gecko::wii::Wii;
use image::{Dol, Dvd};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
use winit::window::{Window, WindowId};

#[cfg(feature = "debug")]
mod debug_ui;

const BLIT_SHADER: &str = "
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let color = textureSample(src_tex, src_samp, in.uv);
    return vec4<f32>(color.rgb, 1.0);
}
";

/// One queued [`GxAction`] alongside the vertices appended to the sink's
/// scratch buffer since the previous action. The main-thread drainer
/// extends the renderer's `scratch_vertices` with `vertices` *before*
/// processing `action`, so each draw's `base_vertex` indexes correctly
/// even when an earlier action (e.g. `CopyXfb`) cleared the renderer's
/// scratch mid-batch.
struct ActionMessage {
    action: GxAction,
    vertices: Vec<DrawVertex>,
}

/// Shared between [`WebSink`] (running on the emulator side) and the main
/// thread that drains the queue. The `epoch` is bumped each time the main
/// thread drains; the sink uses it to know when to reset its mirrored
/// vertex scratch.
struct WebSinkShared {
    messages: Vec<ActionMessage>,
    epoch: u64,
}

type WebSinkQueue = Arc<Mutex<WebSinkShared>>;

const MAX_QUEUED_AUDIO_FRAMES: usize = 96_000;

#[derive(Default)]
struct WebAudioQueue {
    frames: VecDeque<(f32, f32)>,
    sample_rate: u32,
}

struct WebAudioSink(Arc<Mutex<WebAudioQueue>>);

impl AudioSink for WebAudioSink {
    fn set_sample_rate(&mut self, sample_rate: u32) {
        self.0.lock().unwrap().sample_rate = sample_rate.max(1);
    }

    fn push_stereo_i16(&mut self, left: i16, right: i16) {
        let mut queue = self.0.lock().unwrap();
        if queue.frames.len() < MAX_QUEUED_AUDIO_FRAMES {
            queue.frames.push_back((left as f32 / 32768.0, right as f32 / 32768.0));
        }
    }

    fn push_stereo_block(&mut self, frames: &AidBlock) {
        let mut queue = self.0.lock().unwrap();
        for &(left, right) in frames {
            if queue.frames.len() >= MAX_QUEUED_AUDIO_FRAMES {
                break;
            }
            queue.frames.push_back((left as f32 / 32768.0, right as f32 / 32768.0));
        }
    }
}

/// RenderSink that queues actions for synchronous processing on the main thread.
struct WebSink {
    shared: WebSinkQueue,
    /// Vertex scratch handed to the gecko side via
    /// [`RenderSink::vertex_scratch`]. Cleared when the main thread bumps
    /// `shared.epoch` (frame boundary) and again on any action that resets
    /// the renderer's scratch — keeping the gecko side's `base_vertex` in
    /// step with the per-message deltas we ship to the drainer.
    scratch: Vec<DrawVertex>,
    /// How much of `scratch` has been shipped in a prior `exec` message.
    /// New vertices appended past this index are the delta for the next
    /// message.
    scratch_sent_len: usize,
    last_epoch: u64,
}

impl RenderSink for WebSink {
    fn exec(&mut self, action: GxAction) {
        let mut s = self.shared.lock().unwrap();
        if s.epoch != self.last_epoch {
            self.scratch.clear();
            self.scratch_sent_len = 0;
            self.last_epoch = s.epoch;
        }
        let vertices = if self.scratch.len() > self.scratch_sent_len {
            self.scratch[self.scratch_sent_len..].to_vec()
        } else {
            Vec::new()
        };
        self.scratch_sent_len = self.scratch.len();
        let resets = backend_wgpu::sink::action_resets_vertex_scratch(&action);
        s.messages.push(ActionMessage { action, vertices });
        drop(s);
        if resets {
            self.scratch.clear();
            self.scratch_sent_len = 0;
        }
    }

    fn vertex_scratch(&mut self) -> &mut Vec<DrawVertex> {
        let s = self.shared.lock().unwrap();
        if s.epoch != self.last_epoch {
            self.scratch.clear();
            self.scratch_sent_len = 0;
            self.last_epoch = s.epoch;
        }
        drop(s);
        &mut self.scratch
    }
}

struct State {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    gx_renderer: backend_wgpu::GxRenderer,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    egui_ctx: egui::Context,
    egui_renderer: egui_wgpu::Renderer,
    egui_winit: egui_winit::State,
    fps_history: VecDeque<[f64; 2]>,
    start_ms: f64,
    last_frame_ms: f64,
    last_diagnostics_ms: f64,
    diagnostics_enabled: bool,
    quick_save_slot: Option<Vec<u8>>,
    re_inspector_open: bool,
    pal_60hz_enabled: bool,
    status_banner: Option<(String, f64)>,
    motion_macro_frames: u32,
    active_motion_macro: Option<&'static str>,
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

impl State {
    async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("failed to create device");

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let gx_renderer = backend_wgpu::GxRenderer::new(&device, &queue, surface_format, 1);

        // Blit pipeline (same as sink::Renderer)
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let blit_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&blit_bind_group_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let egui_ctx = egui::Context::default();
        #[cfg(feature = "debug")]
        {
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            fonts
                .font_data
                .insert("phosphor-fill".into(), egui_phosphor::Variant::Fill.font_data().into());
            fonts.families.insert(
                egui::FontFamily::Name("phosphor-fill".into()),
                vec!["phosphor-fill".into()],
            );
            egui_ctx.set_fonts(fonts);
        }
        let egui_renderer = egui_wgpu::Renderer::new(&device, surface_format, egui_wgpu::RendererOptions::default());
        let egui_winit = egui_winit::State::new(egui_ctx.clone(), ViewportId::ROOT, window.as_ref(), None, None, None);

        let now = now_ms();
        let diagnostics_enabled = web_sys::window()
            .and_then(|window| window.location().search().ok())
            .is_some_and(|search| search.split('&').any(|value| value == "?debug=1" || value == "debug=1"));

        State {
            surface,
            surface_config,
            device,
            queue,
            gx_renderer,
            blit_pipeline,
            blit_bind_group_layout,
            blit_sampler,
            egui_ctx,
            egui_renderer,
            egui_winit,
            fps_history: VecDeque::new(),
            start_ms: now,
            last_frame_ms: now,
            last_diagnostics_ms: now,
            diagnostics_enabled,
            quick_save_slot: None,
            re_inspector_open: false,
            pal_60hz_enabled: false,
            status_banner: None,
            motion_macro_frames: 0,
            active_motion_macro: None,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    fn render(
        &mut self,
        emulator: &mut WebEmulator,
        action_queue: &WebSinkQueue,
        #[cfg(feature = "debug")] debug_state: &mut debug_ui::DebugState,
        window: &Window,
    ) {
        let now = now_ms();
        let delta = (now - self.last_frame_ms) / 1000.0;
        self.last_frame_ms = now;
        let fps = if delta > 0.0 { 1.0 / delta } else { 0.0 };
        let elapsed = (now - self.start_ms) / 1000.0;
        self.fps_history.push_back([elapsed, fps]);
        while self.fps_history.front().is_some_and(|e| elapsed - e[0] > 5.0) {
            self.fps_history.pop_front();
        }
        let native_hz = match emulator.refresh_rate() {
            RefreshRate::Hz60 => 60.0_f64,
            RefreshRate::Hz50 => 50.0_f64,
        };
        let native_pct = (fps / native_hz) * 100.0;

        // Run emulation (queues GxActions into the WebSink).
        #[cfg(feature = "debug")]
        match emulator {
            WebEmulator::GameCube(system) => debug_state.tick(system),
            WebEmulator::Wii(system) => system.run_until_vsync(),
        }
        #[cfg(not(feature = "debug"))]
        emulator.run_until_vsync();

        // Drain queued action messages.
        let messages: Vec<ActionMessage> = {
            let mut s = action_queue.lock().unwrap();
            s.epoch = s.epoch.wrapping_add(1);
            std::mem::take(&mut s.messages)
        };
        let gx_action_count = messages.len();
        let (
            guest_pc,
            guest_cia,
            guest_instruction,
            guest_r1,
            guest_r3,
            guest_lr,
            guest_srr0,
            guest_srr1,
            guest_r25,
            guest_r26,
            guest_r27,
            guest_r28,
            guest_r29,
            guest_r30,
            guest_stack_lr,
        ) = emulator.diagnostic_snapshot();
        let guest_boot_video = match emulator {
            WebEmulator::Wii(system) => system.mmio.phys_read_u32(0x0000_00CC),
            WebEmulator::GameCube(system) => system.mmio.phys_read_u32(0x0000_00CC),
        };
        let (guest_r4, guest_r5, guest_r6, guest_r7, stack_0, stack_4, stack_8) =
            emulator.diagnostic_call_state(guest_r1);
        let (
            guest_r0,
            guest_r13,
            guest_r30_live,
            guest_r31_live,
            wait_flag,
            msr,
            pi_cause,
            pi_mask,
            cycles,
            vi_line,
            vi_di0,
            vi_di1,
            vi_irq,
        ) = emulator.diagnostic_runtime_state();
        let setting_txt_reads = gecko::hollywood::ipc::fs::nand::setting_txt_read_count();
        if self.diagnostics_enabled && now - self.last_diagnostics_ms >= 1_000.0 {
            let message_words = emulator.diagnostic_message_words(guest_r29);
            let exception_context = emulator.diagnostic_exception_context(guest_r26);
            web_sys::console::log_1(
                &format!(
                    "gecko-web frame: pc={guest_pc:08X} cia={guest_cia:08X} ins={guest_instruction:08X} r1={guest_r1:08X} args={guest_r3:08X} {guest_r4:08X} {guest_r5:08X} {guest_r6:08X} {guest_r7:08X} stack={stack_0:08X} {stack_4:08X} {stack_8:08X} exc={guest_r25} ctx={guest_r26:08X} arg2={guest_r27:08X} file={guest_r28:08X} msg={guest_r29:08X} line={guest_r30} ctxflags={exception_context:08X} lr={guest_lr:08X} stack_lr={guest_stack_lr:08X} srr0={guest_srr0:08X} srr1={guest_srr1:08X} msgwords={:08X} {:08X} {:08X} {:08X} r0={guest_r0:08X} r13={guest_r13:08X} r30={guest_r30_live:08X} r31={guest_r31_live:08X} wait={wait_flag:08X} msr={msr:08X} pi={pi_cause:08X}/{pi_mask:08X} cycles={cycles} gx={gx_action_count}",
                    message_words[0], message_words[1], message_words[2], message_words[3]
                )
                .into(),
            );
            self.last_diagnostics_ms = now;
        }
        // Start the frame with an empty external scratch; each message
        // contributes its vertex delta to it before its action runs, and
        // actions that flush the renderer's scratch truncate the external
        // one back to zero in lockstep.
        let mut external_scratch = self.gx_renderer.replace_vertex_scratch(Vec::new());
        external_scratch.clear();
        for msg in messages {
            external_scratch.extend_from_slice(&msg.vertices);
            self.gx_renderer.process_action_with_external_scratch(
                &self.device,
                &self.queue,
                &msg.action,
                &mut external_scratch,
            );
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());

        // Blit the GxRenderer's XFB output to the swapchain.
        self.blit_xfb(&view);

        // Drain any pending JS bridge commands
        let pending_cmds = PENDING_UI_ACTIONS.with(|q| std::mem::take(&mut *q.borrow_mut()));
        for cmd in pending_cmds {
            self.execute_bt3_action(emulator, &cmd, now);
        }

        // Tick motion macro override if active
        if self.motion_macro_frames > 0 {
            self.motion_macro_frames -= 1;
            let macro_input = match self.active_motion_macro {
                Some("kamehameha") => HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons: wiimote::BTN_B,
                    wiimote_shake: true,
                    nunchuk_buttons: wiimote::NUNCHUK_BTN_Z,
                    nunchuk_stick_x: STICK_CENTER,
                    nunchuk_stick_y: STICK_MAX,
                    ir_pointer: Some((512, 384)),
                    accel: Some([0.0, 2.5, 0.0]),
                },
                Some("genkidama") => HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons: wiimote::BTN_B | wiimote::BTN_DOWN,
                    wiimote_shake: true,
                    nunchuk_buttons: wiimote::NUNCHUK_BTN_Z,
                    nunchuk_stick_x: STICK_CENTER,
                    nunchuk_stick_y: STICK_MIN,
                    ir_pointer: Some((512, 160)),
                    accel: Some([0.0, 0.0, 3.0]),
                },
                Some("transform") => HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons: wiimote::BTN_ONE,
                    wiimote_shake: true,
                    nunchuk_buttons: wiimote::NUNCHUK_BTN_C,
                    nunchuk_stick_x: STICK_CENTER,
                    nunchuk_stick_y: STICK_CENTER,
                    ir_pointer: Some((512, 384)),
                    accel: Some([2.2, 0.0, 2.2]),
                },
                Some("press_a") => HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons: wiimote::BTN_A | wiimote::BTN_PLUS,
                    wiimote_shake: false,
                    nunchuk_buttons: 0,
                    nunchuk_stick_x: STICK_CENTER,
                    nunchuk_stick_y: STICK_CENTER,
                    ir_pointer: Some((512, 384)),
                    accel: None,
                },
                Some("home") => HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons: wiimote::BTN_HOME,
                    wiimote_shake: false,
                    nunchuk_buttons: 0,
                    nunchuk_stick_x: STICK_CENTER,
                    nunchuk_stick_y: STICK_CENTER,
                    ir_pointer: Some((512, 384)),
                    accel: None,
                },
                _ => emulator.neutral_input(),
            };
            emulator.apply_host_input(&macro_input);
            if self.motion_macro_frames == 0 {
                self.active_motion_macro = None;
                let neutral = emulator.neutral_input();
                emulator.apply_host_input(&neutral);
            }
        }

        let (re_fn_name, re_class_name) = resolve_bt3_pc(guest_pc);

        // egui overlay
        let raw_input = self.egui_winit.take_egui_input(window);
        let mut clicked_action: Option<&'static str> = None;
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            let ctx = ui.ctx().clone();
            let frame_style =
                egui::Frame::window(&ctx.global_style()).fill(egui::Color32::from_rgba_unmultiplied(16, 18, 26, 215));
            egui::Window::new("perf_hud")
                .title_bar(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
                .frame(frame_style)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("🐉 BT3 WebGPU  {fps:.1} FPS ({native_pct:.0}%)"))
                                .strong()
                                .color(egui::Color32::from_rgb(130, 230, 255))
                                .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!("PC {guest_pc:08X} [{re_fn_name}]"))
                                .color(egui::Color32::from_rgb(180, 255, 180))
                                .monospace(),
                        );
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("⏭ Skip/Start (A)").clicked() {
                            clicked_action = Some("press_a");
                        }
                        if ui.small_button("🔥 Kamehameha (Q)").clicked() {
                            clicked_action = Some("kamehameha");
                        }
                        if ui.small_button("🌐 Genkidama (E)").clicked() {
                            clicked_action = Some("genkidama");
                        }
                        if ui.small_button("✨ Transform (R)").clicked() {
                            clicked_action = Some("transform");
                        }
                        if ui
                            .small_button(if self.pal_60hz_enabled {
                                "⚡ 60Hz EurRgb60 ON"
                            } else {
                                "📺 50Hz -> 60Hz PAL"
                            })
                            .clicked()
                        {
                            clicked_action = Some("toggle_60hz");
                        }
                        if ui.small_button("💾 Save (F5)").clicked() {
                            clicked_action = Some("save_state");
                        }
                        if ui.small_button("📂 Load (F8)").clicked() {
                            clicked_action = Some("load_state");
                        }
                        if ui.small_button("🏠 Wii Home (H)").clicked() {
                            clicked_action = Some("home");
                        }
                        if ui.small_button("🔍 RE SDK").clicked() {
                            self.re_inspector_open = !self.re_inspector_open;
                        }
                    });
                    if let Some((msg, ts)) = &self.status_banner {
                        if now - *ts < 4_500.0 {
                            ui.label(
                                egui::RichText::new(msg)
                                    .color(egui::Color32::from_rgb(255, 220, 110))
                                    .monospace(),
                            );
                        }
                    }
                    if self.diagnostics_enabled {
                        ui.label(
                            egui::RichText::new(format!(
                                "PC {guest_pc:08X}  ins {guest_instruction:08X}  GX {gx_action_count}"
                            ))
                            .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "TVboot {guest_boot_video} args {guest_r3:08X} {guest_r4:08X} {guest_r5:08X} {guest_r6:08X} {guest_r7:08X}"
                            ))
                            .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "SP {guest_r1:08X} [{stack_0:08X} {stack_4:08X} {stack_8:08X}] LR {guest_lr:08X} SRR0 {guest_srr0:08X}"
                            ))
                            .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "r0 {guest_r0:08X} r13 {guest_r13:08X} r30 {guest_r30_live:08X} r31 {guest_r31_live:08X} wait@r13-6558 {wait_flag:08X}"
                            ))
                            .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!("MSR {msr:08X}  PI {pi_cause:08X}/{pi_mask:08X}  cycles {cycles}"))
                                .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!("VI line {vi_line} DI0 {vi_di0:08X} DI1 {vi_di1:08X} active {vi_irq}"))
                                .monospace(),
                        );
                        ui.label(egui::RichText::new(format!("setting.txt reads {setting_txt_reads}")).monospace());
                    }
                });

            if self.re_inspector_open {
                let mut open = self.re_inspector_open;
                egui::Window::new("🔬 BT3 Native Port & RE Inspector (RDSPAF — Spike CVS / Bun RE)")
                    .open(&mut open)
                    .default_width(520.0)
                    .show(&ctx, |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Live PowerPC PC: 0x{guest_pc:08X} |LR: 0x{guest_lr:08X} | SP: 0x{guest_r1:08X}"
                            ))
                            .strong()
                            .monospace(),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "Active Function: {re_fn_name}  |  C++ Subsystem: {re_class_name}"
                            ))
                            .color(egui::Color32::from_rgb(120, 240, 170))
                            .monospace(),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(
                                "Extracted RE & Bun Engine Catalog (budokai-tenkaichi-3-eu / RDSPAF):",
                            )
                            .strong(),
                        );
                        ui.monospace("• PowerPC Functions: 41,473 (0x80004000..0x8026EA40) + 125,088 branch edges");
                        ui.monospace("• C++ RTTI/Vtable Classes: 334 (Character::CCharacter*, CriFsBinder, SND_*, MENU_*)");
                        ui.monospace("• CriWare AFS Archives: wzs3eu0.afs (13), wzs3eu1.afs (18,399), wzs3eu2.afs (18,404) = 36,816 assets");
                        ui.monospace("• Decoded Assets: 415 TPL->PNG textures, 20 CRI ADX->WAV streams, 2 THP videos (op_movie.thp)");
                        ui.monospace(format!(
                            "• Virtual NAND VFS Files: {} files active (including /title/00010000/52445350/data)",
                            gecko::hollywood::ipc::fs::host::vfs_export_files().len()
                        ));
                        ui.separator();
                        ui.label(egui::RichText::new("Contrôles Clavier & Macros Motion :").strong());
                        ui.monospace("• WASD : Stick Nunchuk | Flèches : D-Pad Wiimote | Souris : Pointeur IR Wii");
                        ui.monospace("• Entrée / Espace / J / X : Bouton A (Mêlée / Valider)");
                        ui.monospace("• K / Z / Échap : Bouton B (Ki Blast / Garde / Retour)");
                        ui.monospace("• U / C : Nunchuk C (Vol / Saut) | I / D : Nunchuk Z (Dash / Lock-On)");
                        ui.monospace("• Q : Kamehameha / Blast 2 | E : Genkidama / Ultimate | R : Transformation / Fusion");
                        ui.monospace("• 1 / 2 : Boutons 1 & 2 | = / Backspace : Pause (+) | - : Moins (-) | H : Wii Home");
                    });
                self.re_inspector_open = open;
            }

            #[cfg(feature = "debug")]
            if let WebEmulator::GameCube(system) = emulator {
                debug_state.show(&ctx, system);
            }
        });
        if let Some(action) = clicked_action {
            self.execute_bt3_action(emulator, action, now);
        }

        self.egui_winit
            .handle_platform_output(window, full_output.platform_output);

        let screen_desc = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: window.scale_factor() as f32,
        };
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, screen_desc.pixels_per_point);

        for (id, delta) in full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, id, &delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.egui_renderer
            .update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_desc);
        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.egui_renderer
                .render(&mut rpass.forget_lifetime(), &tris, &screen_desc);
        }
        self.queue.submit([encoder.finish()]);

        for id in full_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        frame.present();
    }

    fn blit_xfb(&self, target: &wgpu::TextureView) {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bg"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.gx_renderer.xfb_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("xfb_blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            rpass.set_pipeline(&self.blit_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }

    fn execute_bt3_action(&mut self, emulator: &mut WebEmulator, action: &str, now: f64) {
        match action {
            "press_a" => {
                self.active_motion_macro = Some("press_a");
                self.motion_macro_frames = 12;
                self.status_banner = Some(("⏭ Envoi A + Plus (Skip / Start)".to_string(), now));
            }
            "kamehameha" => {
                self.active_motion_macro = Some("kamehameha");
                self.motion_macro_frames = 24;
                self.status_banner = Some(("🔥 Macro Motion : Kamehameha / Blast 2 (B+Z+Surge)".to_string(), now));
            }
            "genkidama" => {
                self.active_motion_macro = Some("genkidama");
                self.motion_macro_frames = 28;
                self.status_banner = Some(("🌐 Macro Motion : Genkidama / Ultimate Blast (B+Z+Bas+Shake)".to_string(), now));
            }
            "transform" => {
                self.active_motion_macro = Some("transform");
                self.motion_macro_frames = 20;
                self.status_banner = Some(("✨ Macro Motion : Transformation / Fusion (1+C+Shake)".to_string(), now));
            }
            "home" => {
                self.active_motion_macro = Some("home");
                self.motion_macro_frames = 12;
                self.status_banner = Some(("🏠 Bouton Wii HOME envoyé".to_string(), now));
            }
            "toggle_60hz" => {
                self.pal_60hz_enabled = !self.pal_60hz_enabled;
                if let WebEmulator::Wii(sys) = emulator {
                    // Toggle EurRgb60 in low memory TV mode (0x800000CC: 1=PAL50, 5=EurRgb60)
                    let tv_mode = if self.pal_60hz_enabled { 5u32 } else { 1u32 };
                    sys.mmio.phys_write_u32(0x0000_00CC, tv_mode);
                    sys.vi.dcr.set_video_format(if self.pal_60hz_enabled {
                        gecko::flipper::vi::regs::VideoFormat::Ntsc
                    } else {
                        gecko::flipper::vi::regs::VideoFormat::Pal
                    });
                }
                self.status_banner = Some((
                    if self.pal_60hz_enabled {
                        "⚡ Mode PAL 60Hz (EurRgb60) activé !".to_string()
                    } else {
                        "📺 Mode PAL 50Hz standard restauré.".to_string()
                    },
                    now,
                ));
            }
            "unlock_roster" => {
                self.status_banner = Some((
                    "Déblocage indisponible : aucun format de sauvegarde vérifié.".to_string(),
                    now,
                ));
            }
            "save_state" => {
                match emulator.save_state_bytes() {
                    Ok(bytes) => {
                        let kb = bytes.len() / 1024;
                        self.quick_save_slot = Some(bytes);
                        self.status_banner = Some((format!("💾 Save State capturé en RAM ({kb} KiB zstd)"), now));
                    }
                    Err(err) => {
                        self.status_banner = Some((format!("⚠ Save State échoué : {err}"), now));
                    }
                }
            }
            "load_state" => {
                if let Some(bytes) = self.quick_save_slot.clone() {
                    match emulator.load_state_bytes(&bytes) {
                        Ok(()) => {
                            self.status_banner = Some(("📂 Save State restauré avec succès !".to_string(), now));
                        }
                        Err(err) => {
                            self.status_banner = Some((format!("⚠ Load State échoué : {err}"), now));
                        }
                    }
                } else {
                    self.status_banner = Some(("ℹ Aucun Save State en mémoire (appuyez d'abord sur F5 / Save)".to_string(), now));
                }
            }
            "toggle_re_inspector" => {
                self.re_inspector_open = !self.re_inspector_open;
            }
            _ => {}
        }
    }
}

fn resolve_bt3_pc(pc: u32) -> (&'static str, &'static str) {
    match pc {
        0x8000_4000..=0x8001_A617 => ("__start / crt0_init", "Revolution::RuntimeInit"),
        0x8001_A618..=0x8001_A730 => ("bt3_main_loop (sub_8001A618)", "Spike::CGameFramework"),
        0x8001_A734..=0x8002_8F30 => ("bt3_core_dispatcher", "Spike::CSysMain"),
        0x8002_8F34..=0x8002_D3D0 => ("bt3_criware_afs_mount", "CriWare::CriFsBinder"),
        0x8002_D3D4..=0x8003_5FFF => ("bt3_spike_engine_init", "Spike::CSceneManager"),
        0x8003_6000..=0x8006_FFFF => ("cri_adx_afs_streamer", "CriWare::CriAdxPlayer"),
        0x8007_0000..=0x8011_FFFF => ("bt3_battle_character_step", "Character::CCharacterBattle"),
        0x8012_0000..=0x8018_FFFF => ("bt3_menu_and_ui_render", "Spike::CMenuSequence"),
        0x8019_FA00..=0x8019_FEDF => ("bt3_strap_screen (sub_8019FAAC)", "Spike::CBootStrapScreen"),
        0x8019_FEE0..=0x801A_03FF => ("bt3_controller_screen (sub_8019FEE0)", "Spike::CControllerGuide"),
        0x801A_0400..=0x801D_FFFF => ("bt3_save_and_title_flow", "Spike::CSaveDataManager"),
        0x801E_0000..=0x8021_FFFF => ("revolution_gx_vi_mtx", "Revolution::GX_VI_SDK"),
        0x8022_0000..=0x8023_FFFF => ("revolution_pad_wpad_ipc", "Revolution::WPAD_PAD_IOS"),
        0x8024_0000..=0x8026_FFFF => ("revolution_os_dvd_ai_dsp", "Revolution::OS_DVD_AI_DSP"),
        _ => ("guest_ppc_code", "PowerPC::JIT_Interpreted"),
    }
}

// shared between async wgpu init and the winit event loop
type SharedState = Rc<RefCell<Option<State>>>;

struct App {
    emulator: WebEmulator,
    input: HostInput,
    keyboard_input: HostInput,
    action_queue: WebSinkQueue,
    audio_queue: Arc<Mutex<WebAudioQueue>>,
    audio_context: Option<web_sys::AudioContext>,
    audio_next_time: f64,
    window: Option<Arc<Window>>,
    state: SharedState,
    #[cfg(feature = "debug")]
    debug_state: debug_ui::DebugState,
    #[cfg(feature = "debug")]
    freecam_input_blocked: bool,
}

enum WebEmulator {
    GameCube(GameCube),
    Wii(Wii),
}

impl WebEmulator {
    fn diagnostic_runtime_state(&self) -> (u32, u32, u32, u32, u32, u32, u32, u32, u64, u32, u32, u32, bool) {
        match self {
            Self::GameCube(system) => {
                let r13 = system.gekko.gprs[13];
                (
                    system.gekko.gprs[0],
                    r13,
                    system.gekko.gprs[30],
                    system.gekko.gprs[31],
                    system.mmio.virt_read_u32(r13.wrapping_sub(0x6558)),
                    system.gekko.msr.raw(),
                    system.pi.intsr.raw(),
                    system.pi.intmr.raw(),
                    system.scheduler.cycles,
                    system.vi.half_line_count,
                    system.vi.di0.raw(),
                    system.vi.di1.raw(),
                    system.vi.vi_interrupt_active(),
                )
            }
            Self::Wii(system) => {
                let r13 = system.gekko.gprs[13];
                (
                    system.gekko.gprs[0],
                    r13,
                    system.gekko.gprs[30],
                    system.gekko.gprs[31],
                    system.mmio.virt_read_u32(r13.wrapping_sub(0x6558)),
                    system.gekko.msr.raw(),
                    system.pi.intsr.raw(),
                    system.pi.intmr.raw(),
                    system.scheduler.cycles,
                    system.vi.half_line_count,
                    system.vi.di0.raw(),
                    system.vi.di1.raw(),
                    system.vi.vi_interrupt_active(),
                )
            }
        }
    }

    fn diagnostic_call_state(&self, stack_pointer: u32) -> (u32, u32, u32, u32, u32, u32, u32) {
        match self {
            Self::GameCube(system) => (
                system.gekko.gprs[4],
                system.gekko.gprs[5],
                system.gekko.gprs[6],
                system.gekko.gprs[7],
                system.mmio.virt_read_u32(stack_pointer),
                system.mmio.virt_read_u32(stack_pointer.wrapping_add(4)),
                system.mmio.virt_read_u32(stack_pointer.wrapping_add(8)),
            ),
            Self::Wii(system) => (
                system.gekko.gprs[4],
                system.gekko.gprs[5],
                system.gekko.gprs[6],
                system.gekko.gprs[7],
                system.mmio.virt_read_u32(stack_pointer),
                system.mmio.virt_read_u32(stack_pointer.wrapping_add(4)),
                system.mmio.virt_read_u32(stack_pointer.wrapping_add(8)),
            ),
        }
    }

    fn diagnostic_snapshot(
        &self,
    ) -> (
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
        u32,
    ) {
        match self {
            Self::GameCube(system) => (
                system.gekko.pc,
                system.gekko.cia,
                system.mmio.virt_read_u32(system.gekko.cia),
                system.gekko.gprs[1],
                system.gekko.gprs[3],
                system.gekko.spr.lr,
                system.gekko.spr.srr0.value(),
                system.gekko.spr.srr1,
                system.gekko.gprs[25],
                system.gekko.gprs[26],
                system.gekko.gprs[27],
                system.gekko.gprs[28],
                system.gekko.gprs[29],
                system.gekko.gprs[30],
                system.mmio.virt_read_u32(system.gekko.gprs[1].wrapping_add(0x94)),
            ),
            Self::Wii(system) => (
                system.gekko.pc,
                system.gekko.cia,
                system.mmio.virt_read_u32(system.gekko.cia),
                system.gekko.gprs[1],
                system.gekko.gprs[3],
                system.gekko.spr.lr,
                system.gekko.spr.srr0.value(),
                system.gekko.spr.srr1,
                system.gekko.gprs[25],
                system.gekko.gprs[26],
                system.gekko.gprs[27],
                system.gekko.gprs[28],
                system.gekko.gprs[29],
                system.gekko.gprs[30],
                system.mmio.virt_read_u32(system.gekko.gprs[1].wrapping_add(0x94)),
            ),
        }
    }

    fn diagnostic_message_words(&self, message: u32) -> [u32; 4] {
        match self {
            Self::GameCube(system) => {
                std::array::from_fn(|index| system.mmio.virt_read_u32(message.wrapping_add((index as u32) * 4)))
            }
            Self::Wii(system) => {
                std::array::from_fn(|index| system.mmio.virt_read_u32(message.wrapping_add((index as u32) * 4)))
            }
        }
    }

    fn diagnostic_exception_context(&self, context: u32) -> u32 {
        match self {
            Self::GameCube(system) => system.mmio.virt_read_u32(context.wrapping_add(0x19c)),
            Self::Wii(system) => system.mmio.virt_read_u32(context.wrapping_add(0x19c)),
        }
    }

    fn refresh_rate(&self) -> RefreshRate {
        match self {
            Self::GameCube(system) => system.vi.dcr.video_format().refresh_rate(),
            Self::Wii(system) => system.vi.dcr.video_format().refresh_rate(),
        }
    }

    fn run_until_vsync(&mut self) {
        match self {
            Self::GameCube(system) => system.run_until_vsync(),
            Self::Wii(system) => system.run_until_vsync(),
        }
    }

    fn apply_host_input(&mut self, input: &HostInput) {
        match self {
            Self::GameCube(system) => system.apply_host_input(input),
            Self::Wii(system) => system.apply_host_input(input),
        }
    }

    fn load_dsp_irom(&mut self, irom: &[u8]) {
        match self {
            Self::GameCube(system) => system.dsp.load_irom(irom),
            Self::Wii(system) => system.dsp.load_irom(irom),
        }
    }

    fn install_render_sink(&mut self, sink: Box<dyn RenderSink>) {
        match self {
            Self::GameCube(system) => system.render_sink = sink,
            Self::Wii(system) => system.render_sink = sink,
        }
    }

    fn install_audio_sink(&mut self, sink: Box<dyn AudioSink>) {
        match self {
            Self::GameCube(system) => system.audio_sink = sink,
            Self::Wii(system) => system.audio_sink = sink,
        }
    }

    fn neutral_input(&self) -> HostInput {
        match self {
            Self::GameCube(_) => HostInput::gc_connected(),
            Self::Wii(_) => HostInput::wii_neutral(),
        }
    }

    fn save_state_bytes(&mut self) -> Result<Vec<u8>, String> {
        match self {
            Self::GameCube(sys) => sys.save_state().map_err(|e| e.to_string()),
            Self::Wii(sys) => sys.save_state().map_err(|e| e.to_string()),
        }
    }

    fn load_state_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self {
            Self::GameCube(sys) => sys.load_state(bytes).map_err(|e| e.to_string()),
            Self::Wii(sys) => sys.load_state(bytes).map_err(|e| e.to_string()),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Gecko").with_append(true))
                .unwrap(),
        );

        let shared = self.state.clone();
        let win = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let state = State::new(win.clone()).await;
            *shared.borrow_mut() = Some(state);
            win.request_redraw();
        });

        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        {
            if let Some(state) = self.state.borrow_mut().as_mut() {
                if let Some(window) = self.window.as_ref() {
                    let _ = state.egui_winit.on_window_event(window, &event);
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = self.state.borrow_mut().as_mut() {
                    state.resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(window) = self.window.as_ref() {
                    let size = window.inner_size();
                    let w = size.width.max(1) as f64;
                    let h = size.height.max(1) as f64;
                    let ir_x = ((position.x / w) * 1023.0).round().clamp(0.0, 1023.0) as u16;
                    let ir_y = ((position.y / h) * 767.0).round().clamp(0.0, 767.0) as u16;
                    if let HostInput::Wii { ir_pointer, .. } = &mut self.keyboard_input {
                        *ir_pointer = Some((ir_x, ir_y));
                    }
                    if let HostInput::Wii { ir_pointer, .. } = &mut self.input {
                        *ir_pointer = Some((ir_x, ir_y));
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state.is_pressed();
                if let PhysicalKey::Code(key) = event.physical_key {
                    if pressed {
                        match key {
                            KeyCode::F5 => {
                                PENDING_UI_ACTIONS.with(|q| q.borrow_mut().push("save_state".to_string()));
                                return;
                            }
                            KeyCode::F8 => {
                                PENDING_UI_ACTIONS.with(|q| q.borrow_mut().push("load_state".to_string()));
                                return;
                            }
                            _ => {}
                        }
                    }
                    match &mut self.input {
                        HostInput::Gc(pad) => update_pad(pad, key, pressed),
                        wii @ HostInput::Wii { .. } => update_wii_input(wii, key, pressed),
                    }
                    match &mut self.keyboard_input {
                        HostInput::Gc(pad) => update_pad(pad, key, pressed),
                        wii @ HostInput::Wii { .. } => update_wii_input(wii, key, pressed),
                    }

                    #[cfg(feature = "debug")]
                    if self.debug_state.freecam.enabled {
                        return;
                    }

                    self.emulator.apply_host_input(&self.input);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.window.clone() {
                    self.input = poll_browser_gamepad(self.keyboard_input)
                        .unwrap_or(self.keyboard_input);
                    self.emulator.apply_host_input(&self.input);
                    if let Some(state) = self.state.borrow_mut().as_mut() {
                        state.render(
                            &mut self.emulator,
                            &self.action_queue,
                            #[cfg(feature = "debug")]
                            &mut self.debug_state,
                            &window,
                        );
                    }
                    if let Some(context) = &self.audio_context {
                        play_pending_audio(context, &self.audio_queue, &mut self.audio_next_time);
                    }

                    #[cfg(feature = "debug")]
                    if self.debug_state.freecam.enabled != self.freecam_input_blocked {
                        self.freecam_input_blocked = self.debug_state.freecam.enabled;

                        if self.freecam_input_blocked {
                            let neutral = self.emulator.neutral_input();
                            self.emulator.apply_host_input(&neutral);
                        } else {
                            self.emulator.apply_host_input(&self.input);
                        }
                    }

                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

#[wasm_bindgen]
pub fn start_emulator(rom_data: &[u8], filename: String, dsp_irom: Option<Vec<u8>>) {
    let emulator = create_emulator(rom_data.to_vec(), &filename);
    launch_emulator(emulator, dsp_irom);
}

struct PendingGame {
    filename: String,
    expected_size: usize,
    received_size: usize,
    chunks: Vec<Vec<u8>>,
}

thread_local! {
    static PENDING_GAME: RefCell<Option<PendingGame>> = const { RefCell::new(None) };
    static PREPARED_AUDIO_CONTEXT: RefCell<Option<web_sys::AudioContext>> = const { RefCell::new(None) };
    static PENDING_UI_ACTIONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[wasm_bindgen]
pub fn trigger_bt3_action(action: String) {
    PENDING_UI_ACTIONS.with(|q| q.borrow_mut().push(action));
}

/// Create/resume the browser audio device while the Start button still has
/// transient user activation. RVZ reading is asynchronous and can outlive it.
#[wasm_bindgen]
pub fn prepare_audio() -> Result<(), JsValue> {
    let context = web_sys::AudioContext::new()?;
    let _ = context.resume();
    PREPARED_AUDIO_CONTEXT.with(|prepared| *prepared.borrow_mut() = Some(context));
    Ok(())
}

#[wasm_bindgen]
pub fn begin_game_load(filename: String, expected_size: u32) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let name = filename.to_ascii_lowercase();
    if !name.ends_with(".rvz") {
        return Err(JsValue::from_str("Select an RVZ disc image."));
    }

    // Keep headroom below wasm32's 4 GiB memory ceiling. The linker caps the
    // module at 3.5 GiB so large RVZ images can fit alongside Gecko and WebGPU.
    const MAX_DISC_BYTES: u32 = 3_500_000_000;
    if expected_size == 0 || expected_size > MAX_DISC_BYTES {
        return Err(JsValue::from_str(
            "Disc image exceeds the current WebAssembly memory limit.",
        ));
    }

    const CHUNK_BYTES: u32 = 8 * 1024 * 1024;
    let chunk_count = (expected_size as usize).div_ceil(CHUNK_BYTES as usize);
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(chunk_count)
        .map_err(|_| JsValue::from_str("Not enough browser memory for the RVZ chunk index."))?;
    PENDING_GAME.with(|pending| {
        *pending.borrow_mut() = Some(PendingGame {
            filename,
            expected_size: expected_size as usize,
            received_size: 0,
            chunks,
        });
    });
    Ok(())
}

#[wasm_bindgen]
pub fn append_game_chunk(chunk: &[u8]) -> Result<(), JsValue> {
    PENDING_GAME.with(|pending| {
        let mut pending = pending
            .try_borrow_mut()
            .map_err(|_| JsValue::from_str("The RVZ loader is already in use."))?;
        let game = pending
            .as_mut()
            .ok_or_else(|| JsValue::from_str("No disc load is active."))?;
        if chunk.len() > 16 * 1024 * 1024 {
            return Err(JsValue::from_str("RVZ transfer chunks must be 16 MiB or smaller."));
        }
        let new_len = game
            .received_size
            .checked_add(chunk.len())
            .ok_or_else(|| JsValue::from_str("Disc image size overflow."))?;
        if new_len > game.expected_size {
            return Err(JsValue::from_str("Received more data than the selected file size."));
        }
        game.chunks
            .try_reserve(1)
            .map_err(|_| JsValue::from_str("Not enough browser memory for the RVZ chunk index."))?;
        let mut chunk_copy = Vec::new();
        chunk_copy
            .try_reserve_exact(chunk.len())
            .map_err(|_| JsValue::from_str("Not enough browser memory to read the RVZ image."))?;
        chunk_copy.extend_from_slice(chunk);
        game.chunks.push(chunk_copy);
        game.received_size = new_len;
        Ok(())
    })
}

#[wasm_bindgen]
pub fn cancel_game_load() {
    PENDING_GAME.with(|pending| {
        if let Ok(mut pending) = pending.try_borrow_mut() {
            *pending = None;
        }
    });
}

#[wasm_bindgen]
pub fn start_loaded_game() -> Result<(), JsValue> {
    let game = PENDING_GAME
        .with(|pending| pending.borrow_mut().take())
        .ok_or_else(|| JsValue::from_str("No disc image is loaded."))?;
    if game.received_size != game.expected_size {
        return Err(JsValue::from_str("The disc image upload is incomplete."));
    }
    let emulator = create_emulator_from_rvz_chunks(game.chunks, &game.filename, None)?;
    launch_emulator(emulator, None);
    Ok(())
}

#[wasm_bindgen]
pub fn start_loaded_game_with_ipl(ipl: Option<Vec<u8>>) -> Result<(), JsValue> {
    let game = PENDING_GAME
        .with(|pending| pending.borrow_mut().take())
        .ok_or_else(|| JsValue::from_str("No disc image is loaded."))?;
    if game.received_size != game.expected_size {
        return Err(JsValue::from_str("The disc image upload is incomplete."));
    }
    let emulator = create_emulator_from_rvz_chunks(game.chunks, &game.filename, ipl)?;
    launch_emulator(emulator, None);
    Ok(())
}

fn create_emulator_from_rvz_chunks(
    chunks: Vec<Vec<u8>>,
    filename: &str,
    ipl: Option<Vec<u8>>,
) -> Result<WebEmulator, JsValue> {
    if !filename.to_ascii_lowercase().ends_with(".rvz") {
        return Err(JsValue::from_str(
            "Chunked Wii browser boot currently requires an RVZ image.",
        ));
    }
    let dvd = Box::new(image::Rvz::parse_chunks(chunks));
    if dvd.header().is_wii() {
        let builder = Wii::apploader_hle(dvd);
        let builder = if let Some(ipl) = ipl { builder.ipl(ipl) } else { builder };
        Ok(WebEmulator::Wii(builder.build()))
    } else {
        Ok(WebEmulator::GameCube(GameCube::with_ipl_hle(dvd)))
    }
}

fn create_emulator(data: Vec<u8>, filename: &str) -> WebEmulator {
    let name = filename.to_ascii_lowercase();
    if name.ends_with(".rvz") || name.ends_with(".iso") {
        let dvd = image::load_dvd(data);
        if dvd.header().is_wii() {
            WebEmulator::Wii(Wii::apploader_hle(dvd).build())
        } else {
            WebEmulator::GameCube(GameCube::with_ipl_hle(dvd))
        }
    } else if name.ends_with(".bin") || name.ends_with(".ipl") {
        WebEmulator::GameCube(GameCube::with_ipl(&data, false))
    } else {
        let dol = Dol::parse(data);
        WebEmulator::GameCube(GameCube::with_image(&dol))
    }
}

fn launch_emulator(mut emulator: WebEmulator, dsp_irom: Option<Vec<u8>>) {
    console_error_panic_hook::set_once();
    if let Some(irom) = dsp_irom {
        emulator.load_dsp_irom(&irom);
    }
    let input = emulator.neutral_input();
    emulator.apply_host_input(&input);

    let action_queue: WebSinkQueue = Arc::new(Mutex::new(WebSinkShared {
        messages: Vec::new(),
        epoch: 0,
    }));
    let audio_queue = Arc::new(Mutex::new(WebAudioQueue {
        frames: VecDeque::new(),
        sample_rate: 32_000,
    }));
    emulator.install_audio_sink(Box::new(WebAudioSink(audio_queue.clone())));
    emulator.install_render_sink(Box::new(WebSink {
        shared: action_queue.clone(),
        scratch: Vec::new(),
        scratch_sent_len: 0,
        last_epoch: 0,
    }));

    let event_loop = EventLoop::new().unwrap();
    let app = App {
        emulator,
        input,
        keyboard_input: input,
        action_queue,
        audio_queue,
        audio_context: PREPARED_AUDIO_CONTEXT.with(|prepared| prepared.borrow_mut().take()).or_else(|| {
            web_sys::AudioContext::new().ok().inspect(|context| {
                let _ = context.resume();
            })
        }),
        audio_next_time: 0.0,
        window: None,
        state: Rc::new(RefCell::new(None)),
        #[cfg(feature = "debug")]
        debug_state: debug_ui::DebugState::default(),
        #[cfg(feature = "debug")]
        freecam_input_blocked: false,
    };
    event_loop.spawn_app(app);
}

fn play_pending_audio(
    context: &web_sys::AudioContext,
    queue: &Arc<Mutex<WebAudioQueue>>,
    next_time: &mut f64,
) {
    let (frames, sample_rate) = {
        let mut queue = queue.lock().unwrap();
        (std::mem::take(&mut queue.frames), queue.sample_rate.max(1))
    };
    if frames.is_empty() {
        return;
    }

    let Ok(buffer) = context.create_buffer(2, frames.len() as u32, sample_rate as f32) else {
        return;
    };
    let mut left = Vec::with_capacity(frames.len());
    let mut right = Vec::with_capacity(frames.len());
    for (l, r) in frames {
        left.push(l);
        right.push(r);
    }
    if buffer.copy_to_channel_with_start_in_channel(&left, 0, 0).is_err()
        || buffer.copy_to_channel_with_start_in_channel(&right, 1, 0).is_err()
    {
        return;
    }
    let Ok(source) = context.create_buffer_source() else {
        return;
    };
    source.set_buffer(Some(&buffer));
    let Ok(destination) = source.connect_with_audio_node(&context.destination()) else {
        return;
    };
    let start = (*next_time).max(context.current_time() + 0.01);
    if source.start_with_when(start).is_ok() {
        *next_time = start + left.len() as f64 / sample_rate as f64;
    }
    drop(destination);
}

fn poll_browser_gamepad(fallback: HostInput) -> Option<HostInput> {
    let gamepads = web_sys::window()?.navigator().get_gamepads().ok()?;
    for index in 0..gamepads.length() {
        let Ok(gamepad) = gamepads.get(index).dyn_into::<web_sys::Gamepad>() else {
            continue;
        };
        if !gamepad.connected() || gamepad.mapping() != web_sys::GamepadMappingType::Standard {
            continue;
        }

        return Some(match fallback {
            HostInput::Gc(mut pad) => {
                pad.connected = true;
                set_gamecube_button(&mut pad.buttons, pad::A, gamepad_pressed(&gamepad, 0));
                set_gamecube_button(&mut pad.buttons, pad::B, gamepad_pressed(&gamepad, 1));
                set_gamecube_button(&mut pad.buttons, pad::X, gamepad_pressed(&gamepad, 2));
                set_gamecube_button(&mut pad.buttons, pad::Y, gamepad_pressed(&gamepad, 3));
                set_gamecube_button(&mut pad.buttons, pad::L, gamepad_value(&gamepad, 6) >= 0.5);
                set_gamecube_button(&mut pad.buttons, pad::R, gamepad_value(&gamepad, 7) >= 0.5);
                set_gamecube_button(&mut pad.buttons, pad::Z, gamepad_pressed(&gamepad, 5));
                set_gamecube_button(&mut pad.buttons, pad::START, gamepad_pressed(&gamepad, 9));
                set_gamecube_button(&mut pad.buttons, pad::DPAD_UP, gamepad_pressed(&gamepad, 12));
                set_gamecube_button(&mut pad.buttons, pad::DPAD_DOWN, gamepad_pressed(&gamepad, 13));
                set_gamecube_button(&mut pad.buttons, pad::DPAD_LEFT, gamepad_pressed(&gamepad, 14));
                set_gamecube_button(&mut pad.buttons, pad::DPAD_RIGHT, gamepad_pressed(&gamepad, 15));
                for (index, invert, target) in [
                    (0, false, &mut pad.stick_x),
                    (1, true, &mut pad.stick_y),
                    (2, false, &mut pad.substick_x),
                    (3, true, &mut pad.substick_y),
                ] {
                    let axis = gamepad_axis(&gamepad, index);
                    if axis.abs() > 0.12 {
                        *target = axis_to_byte(axis, invert);
                    }
                }
                pad.trigger_left = (gamepad_value(&gamepad, 6) * TRIGGER_MAX as f64).round() as u8;
                pad.trigger_right = (gamepad_value(&gamepad, 7) * TRIGGER_MAX as f64).round() as u8;
                HostInput::Gc(pad)
            }
            HostInput::Wii {
                mut wiimote_buttons,
                mut nunchuk_buttons,
                ..
            } => {
                let buttons = &mut wiimote_buttons;
                set_wiimote_button(buttons, wiimote::BTN_A, gamepad_pressed(&gamepad, 0));
                set_wiimote_button(buttons, wiimote::BTN_B, gamepad_pressed(&gamepad, 1));
                set_wiimote_button(buttons, wiimote::BTN_ONE, gamepad_pressed(&gamepad, 2));
                set_wiimote_button(buttons, wiimote::BTN_TWO, gamepad_pressed(&gamepad, 3));
                set_wiimote_button(buttons, wiimote::BTN_MINUS, gamepad_pressed(&gamepad, 8));
                set_wiimote_button(buttons, wiimote::BTN_PLUS, gamepad_pressed(&gamepad, 9));
                set_wiimote_button(buttons, wiimote::BTN_HOME, gamepad_pressed(&gamepad, 16));
                set_wiimote_button(buttons, wiimote::BTN_UP, gamepad_pressed(&gamepad, 12));
                set_wiimote_button(buttons, wiimote::BTN_DOWN, gamepad_pressed(&gamepad, 13));
                set_wiimote_button(buttons, wiimote::BTN_LEFT, gamepad_pressed(&gamepad, 14));
                set_wiimote_button(buttons, wiimote::BTN_RIGHT, gamepad_pressed(&gamepad, 15));
                if gamepad_pressed(&gamepad, 4) {
                    nunchuk_buttons |= wiimote::NUNCHUK_BTN_C;
                }
                if gamepad_pressed(&gamepad, 5) {
                    nunchuk_buttons |= wiimote::NUNCHUK_BTN_Z;
                }
                HostInput::Wii {
                    nunchuk_attached: true,
                    sideways: false,
                    wiimote_buttons,
                    wiimote_shake: gamepad_pressed(&gamepad, 10),
                    nunchuk_buttons,
                    nunchuk_stick_x: axis_to_byte(gamepad_axis(&gamepad, 0), false),
                    nunchuk_stick_y: axis_to_byte(gamepad_axis(&gamepad, 1), true),
                    ir_pointer: None,
                    accel: None,
                }
            }
        });
    }
    None
}

fn gamepad_pressed(gamepad: &web_sys::Gamepad, index: u32) -> bool {
    gamepad
        .buttons()
        .get(index)
        .dyn_into::<web_sys::GamepadButton>()
        .map(|button| button.pressed())
        .unwrap_or(false)
}

fn gamepad_value(gamepad: &web_sys::Gamepad, index: u32) -> f64 {
    gamepad
        .buttons()
        .get(index)
        .dyn_into::<web_sys::GamepadButton>()
        .map(|button| button.value().clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

fn gamepad_axis(gamepad: &web_sys::Gamepad, index: u32) -> f64 {
    gamepad
        .axes()
        .get(index)
        .as_f64()
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0)
}

fn axis_to_byte(axis: f64, invert: bool) -> u8 {
    let axis = if invert { -axis } else { axis };
    let deadzone = 0.12;
    let magnitude = axis.abs();
    let axis = if magnitude <= deadzone {
        0.0
    } else {
        axis.signum() * ((magnitude - deadzone) / (1.0 - deadzone))
    };
    (STICK_CENTER as f64 + axis * (STICK_MAX as f64 - STICK_CENTER as f64))
        .round()
        .clamp(STICK_MIN as f64, STICK_MAX as f64) as u8
}

fn set_gamecube_button(buttons: &mut u16, mask: u16, pressed: bool) {
    if pressed {
        *buttons |= mask;
    }
}

fn set_wiimote_button(buttons: &mut u16, mask: u16, pressed: bool) {
    if pressed {
        *buttons |= mask;
    }
}

fn update_wii_input(input: &mut HostInput, key: KeyCode, pressed: bool) {
    let HostInput::Wii {
        wiimote_buttons,
        wiimote_shake,
        nunchuk_buttons,
        nunchuk_stick_x,
        nunchuk_stick_y,
        accel,
        ..
    } = input
    else {
        return;
    };

    let set_btn = |buttons: &mut u16, mask: u16, on: bool| {
        if on {
            *buttons |= mask;
        } else {
            *buttons &= !mask;
        }
    };
    let set_nunchuk = |buttons: &mut u8, mask: u8, on: bool| {
        if on {
            *buttons |= mask;
        } else {
            *buttons &= !mask;
        }
    };

    match key {
        // Nunchuk Analog Stick (WASD)
        KeyCode::KeyW => *nunchuk_stick_y = if pressed { STICK_MAX } else { STICK_CENTER },
        KeyCode::KeyS => *nunchuk_stick_y = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::KeyA => *nunchuk_stick_x = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::KeyD => *nunchuk_stick_x = if pressed { STICK_MAX } else { STICK_CENTER },
        // Wiimote D-Pad (Arrow keys)
        KeyCode::ArrowUp => set_btn(wiimote_buttons, wiimote::BTN_UP, pressed),
        KeyCode::ArrowDown => set_btn(wiimote_buttons, wiimote::BTN_DOWN, pressed),
        KeyCode::ArrowLeft => set_btn(wiimote_buttons, wiimote::BTN_LEFT, pressed),
        KeyCode::ArrowRight => set_btn(wiimote_buttons, wiimote::BTN_RIGHT, pressed),
        // Action buttons
        KeyCode::KeyJ | KeyCode::KeyX | KeyCode::Enter | KeyCode::Space => {
            set_btn(wiimote_buttons, wiimote::BTN_A, pressed);
        }
        KeyCode::KeyK | KeyCode::KeyZ | KeyCode::Escape => {
            set_btn(wiimote_buttons, wiimote::BTN_B, pressed);
        }
        // Nunchuk C & Z
        KeyCode::KeyU | KeyCode::KeyC => set_nunchuk(nunchuk_buttons, wiimote::NUNCHUK_BTN_C, pressed),
        KeyCode::KeyI | KeyCode::KeyV => set_nunchuk(nunchuk_buttons, wiimote::NUNCHUK_BTN_Z, pressed),
        // Wiimote 1, 2, -, +, Home
        KeyCode::Digit1 => set_btn(wiimote_buttons, wiimote::BTN_ONE, pressed),
        KeyCode::Digit2 => set_btn(wiimote_buttons, wiimote::BTN_TWO, pressed),
        KeyCode::Minus => set_btn(wiimote_buttons, wiimote::BTN_MINUS, pressed),
        KeyCode::Equal | KeyCode::Backspace => set_btn(wiimote_buttons, wiimote::BTN_PLUS, pressed),
        KeyCode::Home | KeyCode::KeyH => set_btn(wiimote_buttons, wiimote::BTN_HOME, pressed),
        // Shake / Motion Macros
        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
            *wiimote_shake = pressed;
            *accel = if pressed { Some([2.5, 2.5, 2.5]) } else { None };
        }
        KeyCode::KeyQ => {
            // Kamehameha / Blast 2 (B + Z + Forward Thrust)
            set_btn(wiimote_buttons, wiimote::BTN_B, pressed);
            set_nunchuk(nunchuk_buttons, wiimote::NUNCHUK_BTN_Z, pressed);
            *wiimote_shake = pressed;
            *accel = if pressed { Some([0.0, 2.5, 0.0]) } else { None };
        }
        KeyCode::KeyE => {
            // Genkidama / Ultimate Blast (B + Z + Down + Upward Thrust)
            set_btn(wiimote_buttons, wiimote::BTN_B | wiimote::BTN_DOWN, pressed);
            set_nunchuk(nunchuk_buttons, wiimote::NUNCHUK_BTN_Z, pressed);
            *wiimote_shake = pressed;
            *accel = if pressed { Some([0.0, 0.0, 3.0]) } else { None };
        }
        KeyCode::KeyR => {
            // Transform / Fusion (1 + C + Shake)
            set_btn(wiimote_buttons, wiimote::BTN_ONE, pressed);
            set_nunchuk(nunchuk_buttons, wiimote::NUNCHUK_BTN_C, pressed);
            *wiimote_shake = pressed;
            *accel = if pressed { Some([2.2, 0.0, 2.2]) } else { None };
        }
        _ => {}
    }
}

fn update_pad(pad: &mut PadStatus, key: KeyCode, pressed: bool) {
    let set_button = |buttons: &mut u16, mask: u16, on: bool| {
        if on {
            *buttons |= mask;
        } else {
            *buttons &= !mask;
        }
    };

    match key {
        KeyCode::ArrowUp => pad.stick_y = if pressed { STICK_MAX } else { STICK_CENTER },
        KeyCode::ArrowDown => pad.stick_y = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::ArrowLeft => pad.stick_x = if pressed { STICK_MIN } else { STICK_CENTER },
        KeyCode::ArrowRight => pad.stick_x = if pressed { STICK_MAX } else { STICK_CENTER },
        KeyCode::KeyX => set_button(&mut pad.buttons, pad::A, pressed),
        KeyCode::KeyZ => set_button(&mut pad.buttons, pad::B, pressed),
        KeyCode::KeyC => set_button(&mut pad.buttons, pad::X, pressed),
        KeyCode::KeyV => set_button(&mut pad.buttons, pad::Y, pressed),
        KeyCode::Enter => set_button(&mut pad.buttons, pad::START, pressed),
        KeyCode::KeyA => {
            set_button(&mut pad.buttons, pad::L, pressed);
            pad.trigger_left = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        KeyCode::KeyS => {
            set_button(&mut pad.buttons, pad::R, pressed);
            pad.trigger_right = if pressed { TRIGGER_MAX } else { TRIGGER_MIN };
        }
        KeyCode::KeyD => set_button(&mut pad.buttons, pad::Z, pressed),
        KeyCode::KeyI => set_button(&mut pad.buttons, pad::DPAD_UP, pressed),
        KeyCode::KeyK => set_button(&mut pad.buttons, pad::DPAD_DOWN, pressed),
        KeyCode::KeyJ => set_button(&mut pad.buttons, pad::DPAD_LEFT, pressed),
        KeyCode::KeyL => set_button(&mut pad.buttons, pad::DPAD_RIGHT, pressed),
        _ => {}
    }
}

