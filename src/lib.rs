use std::sync::Arc;

use asset::{load::Loadable, AssetPath, Assets};
use bevy_ecs::world::World;
use egui::Visuals;
use egui_tools::EguiRenderer;
use pollster::block_on;
use render::{
    material_impl::{DefaultMaterial, DefaultMaterialInstance},
    UploadedImage, UploadedMesh,
};
use wgpu::{Device, Instance, Surface};
use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::WindowEvent, event_loop::EventLoop,
    window::Window,
};

mod asset;
mod bevy_ecs_ext;
mod egui_tools;
mod engine_lifetime;
mod input;
mod math_type;
mod render;
mod time;
mod wgpu_init;

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("Failed to run app.");
}

struct App {
    instance: wgpu::Instance,
    state: Option<State>,
    window: Option<Arc<Window>>,
}

struct State {
    window: Arc<Window>,
    render_state: RenderState,
    depth_texture: UploadedImage,
    egui_renderer: EguiRenderer,
    egui_scale_factor: f32,
    materials: Assets<DefaultMaterial>,
    material_instances: Assets<DefaultMaterialInstance>,
    meshes: Assets<UploadedMesh>,
    images: Assets<UploadedImage>,
    world: World,
}

struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct PushConstants {
    pub model: [[f32; 4]; 4],
}

unsafe impl bytemuck::Pod for PushConstants {}
unsafe impl bytemuck::Zeroable for PushConstants {}

impl App {
    pub fn new() -> App {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        Self {
            state: None,
            window: None,
            instance,
        }
    }

    pub async fn set_window_and_init(&mut self, window: Window) {
        let window = Arc::new(window);
        let i_width = 1600;
        let i_height = 900;
        let _ = window.request_inner_size(PhysicalSize::new(i_width, i_height));
        let surface = self
            .instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");
        let mut state =
            State::new(&self.instance, surface, window.clone(), i_width, i_height).await;

        state.init();

        window.request_redraw();

        self.window.get_or_insert(window);
        self.state.get_or_insert(state);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();

        block_on(self.set_window_and_init(window));
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = self.state.as_mut().unwrap();
        let window = self.window.as_ref().unwrap();
        state.egui_renderer.handle_input(window, &event);
        if !state.input(&event) {
            match event {
                //Update and Render
                WindowEvent::RedrawRequested => {
                    state.handle_redraw(event_loop);
                    window.request_redraw();
                }

                // Close / Exit
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }

                // Reszie
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size);
                }
                _ => {}
            }
        }
    }
}

impl State {
    pub async fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        window: Arc<Window>,
        width: u32,
        height: u32,
    ) -> State {
        let render_state = RenderState::new(instance, surface, width, height).await;
        let depth_texture = RenderState::create_depth_texture(&render_state.device, width, height);
        let egui_renderer = EguiRenderer::new(
            &render_state.device,
            render_state.config.format,
            None,
            1,
            &window,
        );
        egui_renderer.context().set_visuals(Visuals::light());

        Self {
            window: Arc::clone(&window),
            render_state,
            depth_texture,
            egui_renderer,
            materials: Assets::new(),
            material_instances: Assets::new(),
            meshes: Assets::new(),
            images: Assets::new(),
            egui_scale_factor: 0.8,
            world: World::new(),
        }
    }

    pub fn load_default_material(&mut self) {
        let image =
            UploadedImage::load(AssetPath::Assets("@7ife_l-0.jpg".to_string()), self).unwrap();

        let material = Arc::new(DefaultMaterial::new(self));
        self.materials.insert_with_name("default", material.clone());
        let instance = Arc::new(DefaultMaterial::create_instance(self, material, &image));
        self.material_instances
            .insert_with_name("default", instance);
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.render_state.size = new_size;
            self.render_state.config.width = new_size.width;
            self.render_state.config.height = new_size.height;
            self.render_state
                .surface
                .configure(&self.render_state.device, &self.render_state.config);
            self.depth_texture = RenderState::create_depth_texture(
                &self.render_state.device,
                new_size.width,
                new_size.height,
            );
        }
    }
}
impl RenderState {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(
        instance: &Instance,
        surface: Surface<'static>,
        width: u32,
        height: u32,
    ) -> RenderState {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            // determine how to sync
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self {
            device,
            queue,
            surface,
            config,
            size: PhysicalSize { width, height },
        }
    }
    fn get_window_extend3d(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.config.width.max(1),
            height: self.config.height.max(1),
            depth_or_array_layers: 1,
        }
    }

    fn create_depth_texture(device: &Device, width: u32, height: u32) -> UploadedImage {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            // 4.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual), // 5.
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        UploadedImage {
            size,
            texture,
            view,
            sampler,
        }
    }
}
