use std::{
    collections::HashMap,
    env,
    error::Error,
    fs,
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
};

use bytemuck::{Pod, Zeroable};
use gilrs::{Button, EventType, Gilrs};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use wgpu::util::DeviceExt;
use wii_menu_core::{Channel, MenuAction, MenuCatalog, MenuEffect, MenuModel, Screen};
use wii_menu_runtime::{
    ChannelSession, PackageFormat, PackageRequest, launch_channel_session, load_workspace_catalog,
    package_game, prewarm_first_wii_disc, resolve_workspace_root,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, NamedKey, PhysicalKey},
    window::{Window, WindowId},
};

const DESIGN_WIDTH: f32 = 1280.0;
const DESIGN_HEIGHT: f32 = 720.0;
const SCENE_ASPECT: f32 = DESIGN_WIDTH / DESIGN_HEIGHT;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TextureVertex {
    position: [f32; 2],
    tex_coord: [f32; 2],
}

struct TextureAsset {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

impl TextureVertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture_pipeline: wgpu::RenderPipeline,
    icons: HashMap<String, TextureAsset>,
}

impl GpuState {
    fn new(
        window: Arc<Window>,
        icon_paths: &HashMap<String, PathBuf>,
    ) -> Result<Self, Box<dyn Error>> {
        let backends = selected_backends();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone())?;
        let request_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        };
        let (surface, adapter) = match pollster::block_on(
            instance.request_adapter(&request_options),
        ) {
            Ok(adapter) => (surface, adapter),
            Err(primary_error) if backends != wgpu::Backends::all() => {
                eprintln!(
                    "Wii OS GPU: requested backend unavailable ({primary_error}); retrying with all supported backends"
                );
                let fallback_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..wgpu::InstanceDescriptor::new_without_display_handle()
                });
                let fallback_surface = fallback_instance.create_surface(window.clone())?;
                let adapter = pollster::block_on(fallback_instance.request_adapter(
                    &wgpu::RequestAdapterOptions {
                        compatible_surface: Some(&fallback_surface),
                        power_preference: request_options.power_preference,
                        force_fallback_adapter: false,
                    },
                ))?;
                (fallback_surface, adapter)
            }
            Err(error) => return Err(error.into()),
        };
        let adapter_info = adapter.get_info();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        eprintln!(
            "Wii OS GPU: adapter={} vendor={:04x} device={:04x} backend={:?} type={:?} driver={} ({})",
            adapter_info.name,
            adapter_info.vendor,
            adapter_info.device,
            adapter_info.backend,
            adapter_info.device_type,
            adapter_info.driver,
            adapter_info.driver_info
        );

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wii-menu-native-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wii-menu-native-pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wii-menu-native-texture-layout"),
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
        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("wii-menu-native-texture-pipeline-layout"),
                bind_group_layouts: &[Some(&texture_bind_group_layout)],
                immediate_size: 0,
            });
        let texture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wii-menu-native-texture-shader"),
            source: wgpu::ShaderSource::Wgsl(TEXTURE_SHADER.into()),
        });
        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("wii-menu-native-texture-pipeline"),
            layout: Some(&texture_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &texture_shader,
                entry_point: Some("vs_main"),
                buffers: &[TextureVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &texture_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("wii-menu-native-image-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut icons = HashMap::new();
        for (channel_id, path) in icon_paths {
            let image = match image::open(path) {
                Ok(image) => image.to_rgba8(),
                Err(error) => {
                    eprintln!(
                        "Wii OS native renderer: unable to load channel icon {}: {error}",
                        path.display()
                    );
                    continue;
                }
            };
            let (image_width, image_height) = image.dimensions();
            if image_width == 0 || image_height == 0 {
                continue;
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("wii-menu-native-channel-icon"),
                size: wgpu::Extent3d {
                    width: image_width,
                    height: image_height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                image.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image_width * 4),
                    rows_per_image: Some(image_height),
                },
                wgpu::Extent3d {
                    width: image_width,
                    height: image_height,
                    depth_or_array_layers: 1,
                },
            );
            let view = texture.create_view(&Default::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("wii-menu-native-channel-icon-bind-group"),
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            icons.insert(
                channel_id.clone(),
                TextureAsset {
                    _texture: texture,
                    bind_group,
                },
            );
        }

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            texture_pipeline,
            icons,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, model: &MenuModel, occupied: usize, cursor: Option<(f64, f64)>) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            status => {
                eprintln!("Wii OS native renderer: surface acquisition failed: {status:?}");
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let vertices = scene_vertices(model, occupied, self.config.width, self.config.height, cursor);
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("wii-menu-native-vertices"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("wii-menu-native-frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wii-menu-native-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.035,
                            g: 0.055,
                            b: 0.075,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        let icon_quads = scene_texture_quads(model, self.config.width, self.config.height);
        if !icon_quads.is_empty() {
            let texture_vertices: Vec<TextureVertex> = icon_quads
                .iter()
                .flat_map(|(_, quad)| quad.iter().copied())
                .collect();
            let texture_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("wii-menu-native-channel-icon-vertices"),
                        contents: bytemuck::cast_slice(&texture_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("wii-menu-native-channel-icons"),
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
            pass.set_pipeline(&self.texture_pipeline);
            pass.set_vertex_buffer(0, texture_buffer.slice(..));
            for (index, (channel_id, _)) in icon_quads.iter().enumerate() {
                if let Some(icon) = self.icons.get(channel_id) {
                    pass.set_bind_group(0, &icon.bind_group, &[]);
                    let start = (index * 6) as u32;
                    pass.draw(start..start + 6, 0..1);
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        frame.present();
    }
}

fn selected_backends() -> wgpu::Backends {
    match env::var("WII_OS_GPU_BACKEND").as_deref() {
        Ok("dx12") => wgpu::Backends::DX12,
        Ok("vulkan") => wgpu::Backends::VULKAN,
        Ok("gl") => wgpu::Backends::GL,
        Ok("auto") => wgpu::Backends::all(),
        // Auto detection lets wgpu rank adapters compatible with the window
        // surface, preferring the fastest hardware adapter above.
        _ => wgpu::Backends::all(),
    }
}

struct App {
    model: MenuModel,
    occupied: usize,
    icon_paths: HashMap<String, PathBuf>,
    workspace: PathBuf,
    gpu: Option<GpuState>,
    cursor: Option<(f64, f64)>,
    gamepads: Option<Gilrs>,
    dolphin: Option<ChannelSession>,
    dolphin_window: Option<isize>,
}

#[cfg(target_os = "windows")]
struct DolphinWindowSearch {
    process_id: u32,
    window: windows_sys::Win32::Foundation::HWND,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn find_dolphin_window(
    window: windows_sys::Win32::Foundation::HWND,
    context: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::core::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetWindowThreadProcessId};

    let search = unsafe { &mut *(context as *mut DolphinWindowSearch) };
    let mut process_id = 0;
    unsafe {
        GetWindowThreadProcessId(window, &mut process_id);
    }
    if process_id != search.process_id {
        return 1;
    }
    let mut class_name = [0u16; 32];
    let class_length =
        unsafe { GetClassNameW(window, class_name.as_mut_ptr(), class_name.len() as i32) };
    if class_length > 0
        && class_name[..class_length as usize]
            .iter()
            .copied()
            .eq("DolphinNoGUI".encode_utf16())
    {
        search.window = window;
        return 0;
    }
    1
}

impl App {
    fn apply(&mut self, action: MenuAction) {
        if let Some(MenuEffect::LaunchChannel { channel_id }) = self.model.apply(action) {
            let result = launch_channel_session(
                &self.model,
                &self.workspace,
                &channel_id,
                self.dolphin_parent_window(),
            );
            let title = match result {
                Ok(session) => {
                    let title = session
                        .process_id()
                        .map_or_else(|| "Dolphin intégré à Wii OS".to_owned(), |pid| format!("Dolphin intégré (PID {pid})"));
                    self.dolphin = Some(session);
                    self.dolphin_window = None;
                    title
                }
                Err(error) => {
                    self.model
                        .apply(MenuAction::LaunchFinished { succeeded: false });
                    eprintln!("Wii OS : lancement impossible : {error}");
                    format!("Échec du lancement : {error}")
                }
            };
            if let Some(gpu) = &self.gpu {
                gpu.window.set_title(&format!("Wii OS — {title}"));
                gpu.window.request_redraw();
            }
            return;
        }
        if let Some(gpu) = &self.gpu {
            let title = self
                .model
                .selected_channel()
                .map(|channel| channel.title.as_str())
                .unwrap_or("Emplacement vide");
            gpu.window.set_title(&format!("Wii OS — {title}"));
            gpu.window.request_redraw();
        }
    }

    fn activate_slot(&mut self, width: u32, height: u32, x: f64, y: f64) {
        let width = width as f32;
        let height = height as f32;
        let scene_width = width.min(height * SCENE_ASPECT);
        let scene_height = scene_width / SCENE_ASPECT;
        let left = (width - scene_width) * 0.5;
        let top = (height - scene_height) * 0.5;
        let local_x = ((x as f32 - left) / scene_width).clamp(0.0, 0.9999);
        let local_y = ((y as f32 - top) / scene_height).clamp(0.0, 0.9999);

        if matches!(self.model.screen, Screen::HomeButtonMenu) {
            // Interactive Wii HOME Button Menu buttons
            if local_y >= 0.50 && local_y <= 0.65 {
                if local_x >= 0.22 && local_x <= 0.48 {
                    // Left button: Menu Wii
                    self.apply(MenuAction::ShowHealthAndSafety);
                    return;
                } else if local_x >= 0.52 && local_x <= 0.78 {
                    // Right button: Réinitialiser (Reset)
                    self.apply(MenuAction::Start);
                    return;
                }
            }
            // Dismiss or Close button:
            self.apply(MenuAction::ToggleHomeButtonMenu);
            return;
        }

        if local_y >= 0.70 {
            if local_x < 0.16 {
                self.apply(MenuAction::ChangePage { delta: -1 });
            } else if local_x > 0.84 {
                self.apply(MenuAction::ChangePage { delta: 1 });
            }
            return;
        }
        let col = (local_x * 4.0).floor() as usize;
        let row = ((local_y - 0.035) / 0.225).floor().max(0.0) as usize;
        let slot = (row.min(2) * 4 + col.min(3)).min(11);
        self.apply(MenuAction::FocusSlot { slot });
        self.apply(MenuAction::Select);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Wii OS")
            .with_inner_size(LogicalSize::new(960.0, 540.0));
        let attrs = if let Some(monitor) = event_loop.primary_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let available_width = size.width as f32 * 0.9;
            let available_height = size.height as f32 * 0.9;
            let width = available_width.min(available_height * SCENE_ASPECT);
            let height = width / SCENE_ASPECT;
            attrs.with_inner_size(LogicalSize::new(
                f64::from(width) / scale,
                f64::from(height) / scale,
            ))
        } else {
            attrs
        };
        let attrs = if env::var("WII_OS_FULLSCREEN")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        {
            attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(
                event_loop.primary_monitor(),
            )))
        } else {
            attrs
        };
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("create Wii OS window"),
        );
        self.gpu = Some(GpuState::new(window, &self.icon_paths).expect("initialize wgpu renderer"));
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self
            .gpu
            .as_ref()
            .is_none_or(|gpu| gpu.window.id() != window_id)
        {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                    gpu.window.request_redraw();
                }
                self.resize_dolphin_window();
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.render(&self.model, self.occupied, self.cursor);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x, position.y));
                if let Some(gpu) = &self.gpu {
                    gpu.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) / 30.0,
                };
                if dy > 0.3 {
                    self.apply(MenuAction::ChangePage { delta: -1 });
                } else if dy < -0.3 {
                    self.apply(MenuAction::ChangePage { delta: 1 });
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if button == MouseButton::Left {
                    if let (Some((x, y)), Some(gpu)) = (self.cursor, self.gpu.as_ref()) {
                        let size = gpu.window.inner_size();
                        self.activate_slot(size.width, size.height, x, y);
                    }
                } else if button == MouseButton::Right {
                    // Right click = Button B (Annuler / Retour)
                    self.apply(MenuAction::Back);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.dolphin.is_some() && event.physical_key != PhysicalKey::Code(KeyCode::F11) {
                    return;
                }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::F11) => {
                        if let Some(gpu) = &self.gpu {
                            gpu.window
                                .set_fullscreen(if gpu.window.fullscreen().is_some() {
                                    None
                                } else {
                                    Some(winit::window::Fullscreen::Borderless(
                                        gpu.window.current_monitor(),
                                    ))
                                });
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
                        self.apply(MenuAction::MoveSelection { dx: -1, dy: 0 })
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
                        self.apply(MenuAction::MoveSelection { dx: 1, dy: 0 })
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
                        self.apply(MenuAction::MoveSelection { dx: 0, dy: -1 })
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
                        self.apply(MenuAction::MoveSelection { dx: 0, dy: 1 })
                    }
                    PhysicalKey::Code(KeyCode::PageUp) => {
                        self.apply(MenuAction::ChangePage { delta: -1 })
                    }
                    PhysicalKey::Code(KeyCode::PageDown) => {
                        self.apply(MenuAction::ChangePage { delta: 1 })
                    }
                    _ => match event.logical_key.as_ref() {
                        winit::keyboard::Key::Named(NamedKey::Enter)
                        | winit::keyboard::Key::Named(NamedKey::Space) => {
                            if matches!(&self.model.screen, Screen::Home) {
                                self.apply(MenuAction::Select);
                            } else {
                                self.apply(MenuAction::Start);
                            }
                        }
                        winit::keyboard::Key::Named(NamedKey::Home)
                        | winit::keyboard::Key::Named(NamedKey::Tab)
                        | winit::keyboard::Key::Character("h")
                        | winit::keyboard::Key::Character("H") => {
                            self.apply(MenuAction::ToggleHomeButtonMenu)
                        }
                        winit::keyboard::Key::Named(NamedKey::Escape)
                        | winit::keyboard::Key::Named(NamedKey::Backspace) => {
                            self.apply(MenuAction::Back)
                        }
                        winit::keyboard::Key::Character("1") => {
                            self.apply(MenuAction::Select)
                        }
                        winit::keyboard::Key::Character("2") => {
                            self.apply(MenuAction::OpenSettings)
                        }
                        winit::keyboard::Key::Character("+")
                        | winit::keyboard::Key::Character("=") => {
                            self.apply(MenuAction::Start)
                        }
                        winit::keyboard::Key::Character("-") => {
                            self.apply(MenuAction::Back)
                        }
                        winit::keyboard::Key::Character("s")
                        | winit::keyboard::Key::Character("S") => {
                            self.apply(MenuAction::OpenSettings)
                        }
                        winit::keyboard::Key::Character("p")
                        | winit::keyboard::Key::Character("P") => {
                            if let Some(ch) = self.model.selected_channel().cloned() {
                                for fmt in [PackageFormat::Wasm, PackageFormat::Exe] {
                                    match package_game(
                                        &self.workspace,
                                        &PackageRequest {
                                            channel_id: ch.id.clone(),
                                            format: fmt,
                                            auto_boot_game: true,
                                            video_backend: Some("D3D12".to_string()),
                                        },
                                    ) {
                                        Ok(art) => eprintln!(
                                            "Wii OS: package {} généré -> {}",
                                            art.format, art.entrypoint_path
                                        ),
                                        Err(err) => eprintln!("Wii OS: erreur packaging : {err}"),
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.update_dolphin_window();
        if let Some(session) = self.dolphin.as_mut() {
            match session.try_wait() {
                Ok(Some(status)) => {
                    self.dolphin = None;
                    self.dolphin_window = None;
                    if let Some(gpu) = &self.gpu {
                        gpu.window.set_title("Wii OS");
                        gpu.window.request_redraw();
                    }
                    eprintln!("Wii OS: Dolphin terminé ({status})");
                }
                Ok(None) => {}
                Err(error) => eprintln!("Wii OS: surveillance de Dolphin impossible: {error}"),
            }
        }
        let mut redraw = false;
        let mut toggle_fullscreen = false;
        let mut actions = Vec::new();
        if let Some(gamepads) = self.gamepads.as_mut() {
            while let Some(event) = gamepads.next_event() {
                match event.event {
                    EventType::Connected => {
                        eprintln!(
                            "Wii OS controller detected: {}",
                            gamepads.gamepad(event.id).name()
                        );
                    }
                    EventType::Disconnected => {
                        eprintln!("Wii OS controller disconnected");
                    }
                    EventType::ButtonPressed(Button::DPadLeft, _) => {
                        actions.push(MenuAction::MoveSelection { dx: -1, dy: 0 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::DPadRight, _) => {
                        actions.push(MenuAction::MoveSelection { dx: 1, dy: 0 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::DPadUp, _) => {
                        actions.push(MenuAction::MoveSelection { dx: 0, dy: -1 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::DPadDown, _) => {
                        actions.push(MenuAction::MoveSelection { dx: 0, dy: 1 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::South, _) => {
                        if matches!(&self.model.screen, Screen::Home) {
                            actions.push(MenuAction::Select);
                        } else {
                            actions.push(MenuAction::Start);
                        }
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::East, _) => {
                        actions.push(MenuAction::Back);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::North, _) => {
                        // Button 1 (Wiimote) / Triangle / Y
                        actions.push(MenuAction::Select);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::West, _) => {
                        // Button 2 (Wiimote) / Square / X
                        actions.push(MenuAction::OpenSettings);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::Start, _) => {
                        // Button + / Plus (Wiimote) / Options / Menu
                        actions.push(MenuAction::Start);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::Select, _) => {
                        // Button - / Minus (Wiimote) / Share / View
                        actions.push(MenuAction::OpenSettings);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::LeftTrigger, _)
                    | EventType::ButtonPressed(Button::LeftTrigger2, _) => {
                        actions.push(MenuAction::ChangePage { delta: -1 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::RightTrigger, _)
                    | EventType::ButtonPressed(Button::RightTrigger2, _) => {
                        actions.push(MenuAction::ChangePage { delta: 1 });
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::Mode, _) => {
                        // Xbox Guide / PlayStation Button -> Authentic Wii HOME Menu!
                        actions.push(MenuAction::ToggleHomeButtonMenu);
                        redraw = true;
                    }
                    EventType::ButtonPressed(Button::C, _) | EventType::ButtonPressed(Button::Z, _) => {
                        toggle_fullscreen = true;
                        redraw = true;
                    }
                    _ => {}
                }
            }
        }
        if self.dolphin.is_none() {
            for action in actions {
                self.apply(action);
            }
        }
        if toggle_fullscreen && let Some(gpu) = &self.gpu {
            gpu.window
                .set_fullscreen(if gpu.window.fullscreen().is_some() {
                    None
                } else {
                    Some(winit::window::Fullscreen::Borderless(
                        gpu.window.current_monitor(),
                    ))
                });
        }
        if redraw && let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(8),
        ));
    }
}

impl App {
    fn dolphin_parent_window(&self) -> usize {
        #[cfg(target_os = "windows")]
        if let Some(gpu) = &self.gpu
            && let Ok(RawWindowHandle::Win32(handle)) = gpu.window.window_handle().map(|value| value.as_raw())
        {
            return handle.hwnd.get() as usize;
        }
        0
    }

    fn update_dolphin_window(&mut self) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos};
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                EnumWindows, GWL_STYLE, SetForegroundWindow, SetParent, SetWindowLongPtrW,
                WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            };

            let Some(session) = &self.dolphin else { return };
            let Some(gpu) = &self.gpu else { return };
            let Ok(RawWindowHandle::Win32(parent_handle)) =
                gpu.window.window_handle().map(|h| h.as_raw())
            else {
                return;
            };
            let parent = parent_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            if let Some(window) = session.render_window() {
                let window_key = window as isize;
                if self.dolphin_window != Some(window_key) {
                    self.dolphin_window = Some(window_key);
                    let mut rect = windows_sys::Win32::Foundation::RECT::default();
                    unsafe {
                        windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(parent, &mut rect);
                        SetWindowPos(
                            window as windows_sys::Win32::Foundation::HWND,
                            std::ptr::null_mut(),
                            0,
                            0,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_SHOWWINDOW,
                        );
                    }
                }
                return;
            }
            let Some(process_id) = session.process_id() else { return };
            let mut search = DolphinWindowSearch {
                process_id,
                window: std::ptr::null_mut(),
            };
            unsafe {
                EnumWindows(
                    Some(find_dolphin_window),
                    &mut search as *mut DolphinWindowSearch as isize,
                );
            }
            let window = search.window;
            if window.is_null() {
                return;
            }
            let window_key = window as isize;
            if self.dolphin_window == Some(window_key) {
                return;
            }

            let style = unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(window, GWL_STYLE)
            };
            let child_style =
                (style & !(WS_OVERLAPPEDWINDOW as isize)) | WS_CHILD as isize | WS_VISIBLE as isize;
            unsafe {
                SetParent(window, parent);
                SetWindowLongPtrW(window, GWL_STYLE, child_style);
                SetForegroundWindow(window);
            }
            self.dolphin_window = Some(window_key);
            self.resize_dolphin_window();
        }
    }

    fn resize_dolphin_window(&mut self) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::{
                Foundation::RECT,
                UI::WindowsAndMessaging::{
                    GetClientRect, SW_SHOW, SWP_FRAMECHANGED, SWP_NOZORDER, SWP_SHOWWINDOW,
                    SetWindowPos, ShowWindow,
                },
            };
            let (Some(window), Some(gpu)) = (self.dolphin_window, self.gpu.as_ref()) else {
                return;
            };
            let Ok(RawWindowHandle::Win32(parent_handle)) =
                gpu.window.window_handle().map(|h| h.as_raw())
            else {
                return;
            };
            let parent = parent_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            let child = window as windows_sys::Win32::Foundation::HWND;
            let mut rect = RECT::default();
            if unsafe { GetClientRect(parent, &mut rect) } == 0 {
                return;
            }
            unsafe {
                SetWindowPos(
                    child,
                    std::ptr::null_mut(),
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
                );
                ShowWindow(child, SW_SHOW);
            }
        }
    }
}

fn scene_vertices(
    model: &MenuModel,
    occupied: usize,
    width: u32,
    height: u32,
    cursor: Option<(f64, f64)>,
) -> Vec<Vertex> {
    let mut out = Vec::with_capacity(6 * 50);
    let viewport_width = width.max(1) as f32;
    let viewport_height = height.max(1) as f32;
    let scene_width = viewport_width.min(viewport_height * SCENE_ASPECT);
    let scene_height = scene_width / SCENE_ASPECT;
    let offset_x = (viewport_width - scene_width) * 0.5;
    let offset_y = (viewport_height - scene_height) * 0.5;
    let page = model.page(model.current_page()).unwrap_or(&[]);

    let mut rect = |x: f32, y: f32, w: f32, h: f32, color: [f32; 4]| {
        let x0 = 2.0 * (offset_x + x / DESIGN_WIDTH * scene_width) / viewport_width - 1.0;
        let x1 = 2.0 * (offset_x + (x + w) / DESIGN_WIDTH * scene_width) / viewport_width - 1.0;
        let y0 = 1.0 - 2.0 * (offset_y + y / DESIGN_HEIGHT * scene_height) / viewport_height;
        let y1 = 1.0 - 2.0 * (offset_y + (y + h) / DESIGN_HEIGHT * scene_height) / viewport_height;
        out.extend_from_slice(&[
            Vertex {
                position: [x0, y0],
                color,
            },
            Vertex {
                position: [x0, y1],
                color,
            },
            Vertex {
                position: [x1, y1],
                color,
            },
            Vertex {
                position: [x0, y0],
                color,
            },
            Vertex {
                position: [x1, y1],
                color,
            },
            Vertex {
                position: [x1, y0],
                color,
            },
        ]);
    };

    rect(
        0.0,
        0.0,
        DESIGN_WIDTH,
        DESIGN_HEIGHT,
        [0.88, 0.91, 0.93, 1.0],
    );
    rect(0.0, 501.0, DESIGN_WIDTH, 219.0, [0.79, 0.82, 0.85, 1.0]);
    rect(0.0, 498.0, DESIGN_WIDTH, 3.0, [0.38, 0.76, 0.86, 1.0]);
    rect(40.0, 555.0, 90.0, 90.0, [0.87, 0.96, 0.98, 1.0]);
    rect(1150.0, 555.0, 90.0, 90.0, [0.87, 0.96, 0.98, 1.0]);
    rect(320.0, 590.0, 640.0, 25.0, [0.92, 0.95, 0.96, 1.0]);

    let page_occupied = page
        .iter()
        .filter(|channel| channel.is_some())
        .count()
        .min(occupied);
    for row in 0..3 {
        for col in 0..4 {
            let slot = row * 4 + col;
            let x = 31.0 + col as f32 * 320.0;
            let y = 18.0 + row as f32 * 160.0;
            let is_occupied = slot < page_occupied;
            let selected = slot == model.selected_slot();
            rect(
                x - 3.0,
                y - 3.0,
                264.0,
                148.0,
                if selected {
                    [0.30, 0.72, 0.84, 1.0]
                } else {
                    [0.70, 0.74, 0.77, 1.0]
                },
            );
            rect(
                x,
                y,
                258.0,
                142.0,
                if is_occupied {
                    [0.97, 0.98, 0.98, 1.0]
                } else {
                    [0.89, 0.91, 0.92, 1.0]
                },
            );
            if is_occupied {
                let colors = [
                    [0.38, 0.76, 0.86, 1.0],
                    [0.56, 0.82, 0.73, 1.0],
                    [0.91, 0.76, 0.49, 1.0],
                ];
                rect(x + 12.0, y + 16.0, 234.0, 82.0, colors[slot % colors.len()]);
                rect(x + 28.0, y + 112.0, 104.0, 8.0, [0.63, 0.68, 0.71, 1.0]);
                rect(x + 142.0, y + 112.0, 62.0, 8.0, [0.78, 0.81, 0.83, 1.0]);
            } else {
                rect(x + 18.0, y + 18.0, 222.0, 106.0, [0.93, 0.94, 0.95, 1.0]);
            }
        }
    }

    // Authentic Wii Home Button Menu overlay
    if matches!(model.screen, Screen::HomeButtonMenu) {
        // Semi-transparent darkened background
        rect(0.0, 0.0, DESIGN_WIDTH, DESIGN_HEIGHT, [0.05, 0.08, 0.12, 0.85]);

        // Main modal container (Frutiger Aero styled rounded card)
        rect(240.0, 80.0, 800.0, 560.0, [0.96, 0.98, 0.99, 1.0]);
        // Top banner: Wii Cyan blue
        rect(240.0, 80.0, 800.0, 64.0, [0.22, 0.68, 0.88, 1.0]);
        rect(240.0, 144.0, 800.0, 3.0, [0.15, 0.55, 0.75, 1.0]);

        // 4 Wiimote Player Slots with battery indicators
        for p in 0..4 {
            let bx = 280.0 + p as f32 * 180.0;
            let by = 180.0;
            rect(bx, by, 160.0, 140.0, [0.90, 0.93, 0.95, 1.0]);
            // Player badge (P1 cyan, P2 orange, P3 green, P4 yellow)
            let p_colors = [
                [0.18, 0.65, 0.92, 1.0],
                [0.92, 0.55, 0.20, 1.0],
                [0.25, 0.75, 0.40, 1.0],
                [0.90, 0.78, 0.22, 1.0],
            ];
            rect(bx + 12.0, by + 12.0, 36.0, 36.0, p_colors[p]);
            // 4 battery LED segments
            for b in 0..4 {
                let led_x = bx + 58.0 + b as f32 * 22.0;
                let led_y = by + 20.0;
                let is_lit = if p == 0 { true } else if p == 1 { b < 3 } else { b < 2 };
                rect(
                    led_x,
                    led_y,
                    16.0,
                    18.0,
                    if is_lit {
                        [0.15, 0.70, 0.95, 1.0]
                    } else {
                        [0.72, 0.76, 0.78, 1.0]
                    },
                );
            }
            // Controller status indicator bars
            rect(bx + 20.0, by + 75.0, 120.0, 14.0, [0.65, 0.70, 0.75, 1.0]);
            rect(bx + 35.0, by + 100.0, 90.0, 12.0, [0.78, 0.82, 0.85, 1.0]);
        }

        // Action Button: "Menu Wii" (Left)
        rect(280.0, 360.0, 340.0, 100.0, [0.22, 0.68, 0.88, 1.0]);
        rect(284.0, 364.0, 332.0, 92.0, [0.98, 0.99, 1.0, 1.0]);
        rect(330.0, 400.0, 240.0, 20.0, [0.20, 0.60, 0.82, 1.0]);

        // Action Button: "Réinitialiser" (Right)
        rect(660.0, 360.0, 340.0, 100.0, [0.70, 0.75, 0.80, 1.0]);
        rect(664.0, 364.0, 332.0, 92.0, [0.95, 0.96, 0.98, 1.0]);
        rect(710.0, 400.0, 240.0, 20.0, [0.50, 0.55, 0.60, 1.0]);

        // Bottom Button: "Fermer"
        rect(490.0, 520.0, 300.0, 70.0, [0.80, 0.84, 0.88, 1.0]);
        rect(494.0, 524.0, 292.0, 62.0, [0.97, 0.98, 0.99, 1.0]);
        rect(550.0, 550.0, 180.0, 14.0, [0.45, 0.50, 0.55, 1.0]);
    }

    // Authentic Wiimote Infrared Pointer Cursor
    if let Some((mx, my)) = cursor {
        let norm_x = (mx as f32 - offset_x) / scene_width;
        let norm_y = (my as f32 - offset_y) / scene_height;
        if (0.0..=1.0).contains(&norm_x) && (0.0..=1.0).contains(&norm_y) {
            let cx = norm_x * DESIGN_WIDTH;
            let cy = norm_y * DESIGN_HEIGHT;

            // Drop shadow
            rect(cx + 4.0, cy + 4.0, 20.0, 32.0, [0.0, 0.0, 0.0, 0.35]);
            // Black outline
            rect(cx - 1.0, cy - 1.0, 22.0, 34.0, [0.10, 0.15, 0.20, 0.95]);
            // Main pointer body (Nintendo clean white)
            rect(cx, cy, 20.0, 32.0, [0.98, 0.99, 1.0, 1.0]);
            // Player 1 turquoise / cyan gem
            rect(cx + 4.0, cy + 4.0, 12.0, 12.0, [0.18, 0.72, 0.92, 1.0]);
            // Shiny highlight
            rect(cx + 6.0, cy + 6.0, 4.0, 4.0, [1.0, 1.0, 1.0, 0.95]);
        }
    }

    out
}

fn scene_texture_quads(
    model: &MenuModel,
    width: u32,
    height: u32,
) -> Vec<(String, [TextureVertex; 6])> {
    let viewport_width = width.max(1) as f32;
    let viewport_height = height.max(1) as f32;
    let scene_width = viewport_width.min(viewport_height * SCENE_ASPECT);
    let scene_height = scene_width / SCENE_ASPECT;
    let offset_x = (viewport_width - scene_width) * 0.5;
    let offset_y = (viewport_height - scene_height) * 0.5;
    let mut draws = Vec::new();
    let point = |x: f32, y: f32, uv: [f32; 2]| TextureVertex {
        position: [
            2.0 * (offset_x + x / DESIGN_WIDTH * scene_width) / viewport_width - 1.0,
            1.0 - 2.0 * (offset_y + y / DESIGN_HEIGHT * scene_height) / viewport_height,
        ],
        tex_coord: uv,
    };
    let mut quad = |key: String, x: f32, y: f32, w: f32, h: f32, uv: [[f32; 2]; 2]| {
        let [uv0, uv1] = uv;
        draws.push((
            key,
            [
                point(x, y, [uv0[0], uv0[1]]),
                point(x, y + h, [uv0[0], uv1[1]]),
                point(x + w, y + h, [uv1[0], uv1[1]]),
                point(x, y, [uv0[0], uv0[1]]),
                point(x + w, y + h, [uv1[0], uv1[1]]),
                point(x + w, y, [uv1[0], uv0[1]]),
            ],
        ));
    };

    let page = model.page(model.current_page()).unwrap_or(&[]);
    for slot in 0..12 {
        let col = slot % 4;
        let row = slot / 4;
        let x = 31.0 + col as f32 * 320.0;
        let y = 18.0 + row as f32 * 160.0;
        if let Some(Some(channel)) = page.get(slot) {
            quad(
                channel.id.clone(),
                x,
                y,
                258.0,
                142.0,
                [[0.0, 0.0], [1.0, 1.0]],
            );
        } else {
            // The Wii.JS sprite sheet contains three animated empty-slot frames.
            quad(
                "_asset_channel-spritesheet".into(),
                x,
                y,
                258.0,
                142.0,
                [[0.0, 0.0], [1.0 / 3.0, 1.0]],
            );
            quad(
                "_asset_channel-wiilogo".into(),
                x + 57.0,
                y + 27.0,
                144.0,
                84.0,
                [[0.0, 0.0], [1.0, 1.0]],
            );
        }
    }

    // Use the checked-in Wii menu artwork for the frame and footer controls.
    for slot in 0..12 {
        let col = slot % 4;
        let row = slot / 4;
        let x = 31.0 + col as f32 * 320.0;
        let y = 18.0 + row as f32 * 160.0;
        quad(
            "_asset_channel-border".into(),
            x,
            y,
            258.0,
            142.0,
            [[0.0, 0.0], [1.0, 1.0]],
        );
        if slot == model.selected_slot() {
            quad(
                "_asset_channel-hover".into(),
                x,
                y,
                258.0,
                142.0,
                [[0.0, 0.0], [1.0, 1.0]],
            );
        }
    }
    quad(
        "_asset_bottom-title".into(),
        40.0,
        501.0,
        1200.0,
        219.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    quad(
        "_asset_left-button".into(),
        0.0,
        543.0,
        185.0,
        135.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    quad(
        "_asset_right-button".into(),
        1095.0,
        543.0,
        185.0,
        135.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    quad(
        "_asset_wii-button".into(),
        60.0,
        555.0,
        100.0,
        105.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    quad(
        "_asset_mail-button".into(),
        1120.0,
        555.0,
        100.0,
        105.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    quad(
        "_asset_bottom-bg".into(),
        0.0,
        501.0,
        DESIGN_WIDTH,
        219.0,
        [[0.0, 0.0], [1.0, 1.0]],
    );
    // Keep the footer controls above its background texture.
    drop(quad);
    let footer_start = draws.len().saturating_sub(6);
    draws[footer_start..].rotate_left(5);
    draws
}

fn workspace_root() -> PathBuf {
    resolve_workspace_root()
}

fn resolve_icon_path(workspace: &Path, icon: &str) -> Option<PathBuf> {
    if let Some(relative) = icon.strip_prefix("assets/user/") {
        for candidate in [
            workspace.join("userland/data/wii/assets").join(relative),
            workspace
                .join("wii-os/userland/data/wii/assets")
                .join(relative),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(relative) = icon.strip_prefix("assets/images/") {
        for candidate in [
            workspace.join("frontend/assets/images").join(relative),
            workspace
                .join("wii-os/frontend/assets/images")
                .join(relative),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn load_model() -> Result<(MenuModel, usize, HashMap<String, PathBuf>, Vec<Channel>), Box<dyn Error>>
{
    let workspace = workspace_root();
    let (_, catalog) = if let Some(arg_path) = env::args().nth(1).map(PathBuf::from) {
        let text = fs::read_to_string(&arg_path)?;
        let cat: MenuCatalog = serde_json::from_str(&text)?;
        cat.validate()?;
        (arg_path, cat)
    } else {
        load_workspace_catalog(&workspace)?
    };
    let count = catalog.channels.len();
    let channels = catalog.channels.clone();
    let icon_paths = catalog
        .channels
        .iter()
        .filter_map(|channel| {
            channel
                .icon
                .as_deref()
                .and_then(|icon| resolve_icon_path(&workspace, icon))
                .filter(|path| path.is_file())
                .or_else(|| resolve_icon_path(&workspace, "assets/images/channel-icon.png"))
                .map(|path| (channel.id.clone(), path))
        })
        .collect::<HashMap<_, _>>();
    let asset_names = [
        "bottom-bg.png",
        "bottom-title.png",
        "channel-border.png",
        "channel-hover.png",
        "channel-spritesheet.png",
        "channel-wiilogo.png",
        "left-button.png",
        "right-button.png",
        "wii-button.png",
        "mail-button.png",
    ];
    let mut icon_paths = icon_paths;
    for name in asset_names {
        if let Some(path) = resolve_icon_path(&workspace, &format!("assets/images/{name}")) {
            if path.is_file() {
                icon_paths.insert(format!("_asset_{}", name.trim_end_matches(".png")), path);
            }
        }
    }
    Ok((catalog.into_model()?, count, icon_paths, channels))
}

fn check_packaged_autoboot(model: &mut MenuModel, workspace: &mut PathBuf) -> Option<String> {
    if let Ok(channel_id) = env::var("WII_OS_AUTOBOOT_CHANNEL")
        && !channel_id.trim().is_empty()
    {
        return Some(channel_id);
    }
    let Ok(exe_path) = env::current_exe() else {
        return None;
    };
    let stem = exe_path.file_stem()?.to_string_lossy();
    let boot_json_path = exe_path.parent()?.join(format!("{stem}.boot.json"));
    if !boot_json_path.is_file() {
        return None;
    }
    let Ok(text) = fs::read_to_string(&boot_json_path) else {
        return None;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return None;
    };
    if let Some(fb) = val["workspaceFallback"].as_str() {
        let candidate = PathBuf::from(fb);
        if candidate.is_dir() {
            *workspace = candidate;
        }
    }
    if val["autoBootGame"].as_bool().unwrap_or(false) {
        val["channelId"].as_str().map(str::to_string)
    } else {
        let _ = model.apply(MenuAction::ShowHealthAndSafety);
        None
    }
}

#[allow(dead_code)]
fn main() -> Result<(), Box<dyn Error>> {
    run_native_menu()
}

pub fn run_native_menu() -> Result<(), Box<dyn Error>> {
    let (mut model, occupied, icon_paths, channels) = load_model()?;
    let mut workspace = workspace_root();
    let autoboot = check_packaged_autoboot(&mut model, &mut workspace);
    prewarm_first_wii_disc(&channels, &workspace);
    let event_loop = EventLoop::new()?;
    let gamepads = match Gilrs::new() {
        Ok(gamepads) => Some(gamepads),
        Err(error) => {
            eprintln!("Wii OS controller auto-detection unavailable: {error}");
            None
        }
    };
    let mut app = App {
        model,
        occupied,
        icon_paths,
        workspace,
        gpu: None,
        cursor: None,
        gamepads,
        dolphin: None,
        dolphin_window: None,
    };
    if let Some(channel_id) = autoboot {
        app.model.screen = Screen::Preview { channel_id };
        app.apply(MenuAction::Start);
    }
    event_loop.run_app(&mut app)?;
    Ok(())
}

const SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
"#;

const TEXTURE_SHADER: &str = r#"
@group(0) @binding(0) var channel_image: texture_2d<f32>;
@group(0) @binding(1) var channel_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) tex_coord: vec2<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.tex_coord = tex_coord;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(channel_image, channel_sampler, input.tex_coord);
}
"#;
