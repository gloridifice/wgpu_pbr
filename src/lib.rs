use bevy_ecs::system::{Resource, RunSystemOnce};
use bevy_ecs::world::World;
use bevy_ecs::{change_detection::Mut, system::IntoSystem};
use egui_tools::EguiRenderer;
use pollster::block_on;
use std::sync::Arc;
use wgpu::{Instance, Surface};
use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::WindowEvent, event_loop::EventLoop,
    window::Window,
};

mod asset;
mod editor;
mod egui_tools;
mod engine;
mod engine_lifetime;
mod macro_utils;
mod math_type;
mod render;
pub mod wgpu_init;

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
    world: World,
}

#[derive(Resource)]
pub struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

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
        state.world.insert_resource(MainWindow(Arc::clone(&window)));

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
        state.egui_renderer_mut().handle_input(window, &event);
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
        let egui_renderer = EguiRenderer::new(
            &render_state.device,
            render_state.config.format,
            None,
            1,
            &window,
        );
        let mut world = World::new();
        world.insert_resource(render_state);
        world.insert_resource(egui_renderer);

        Self {
            window: Arc::clone(&window),
            // materials: Assets::new(),
            // material_instances: Assets::new(),
            // meshes: Assets::new(),
            // images: Assets::new(),
            world,
        }
    }

    fn run_system_cached<T, Out: 'static, Marker>(&mut self, system: T)
    where
        T: IntoSystem<(), Out, Marker> + 'static,
    {
        self.world.run_system_cached(system).unwrap();
    }

    fn run_system_once<T, Out: 'static, Marker>(&mut self, system: T)
    where
        T: IntoSystem<(), Out, Marker> + 'static,
    {
        self.world.run_system_once(system).unwrap();
    }

    pub fn render_state(&self) -> &RenderState {
        self.world.resource::<RenderState>()
    }

    pub fn egui_renderer_mut(&mut self) -> Mut<'_, EguiRenderer> {
        self.world.resource_mut::<EguiRenderer>()
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        let mut rs = self.world.resource_mut::<RenderState>();
        if new_size.width > 0 && new_size.height > 0 {
            rs.size = new_size;
            rs.config.width = new_size.width;
            rs.config.height = new_size.height;
            rs.surface.configure(&rs.device, &rs.config);
            // self.depth_texture =
            //     RenderState::create_depth_texture(&rs.device, new_size.width, new_size.height, Some(wgpu::CompareFunction::LessEqual));
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

    #[allow(unused)]
    fn get_window_extend3d(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.config.width.max(1),
            height: self.config.height.max(1),
            depth_or_array_layers: 1,
        }
    }
}

#[derive(Resource, Clone)]
pub struct MainWindow(pub Arc<Window>);
