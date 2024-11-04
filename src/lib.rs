use std::sync::Arc;

use asset::{load::Loadable, AssetPath, Assets};
use cgmath::{InnerSpace, Vector3};
use input::INPUT;
use render::{
    camera::{Camera, CameraUniform},
    material_creations, DrawAble, DrawContext, Material, MaterialInstance, UploadedImage,
    UploadedMesh, Vertex,
};
use time::TIME;
use wgpu::{util::DeviceExt, BindGroupEntry, BindGroupLayout, RenderPass};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
    window::WindowBuilder,
};

mod asset;
mod input;
mod render;
mod time;

pub async fn run() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = State::new(&window).await;
    state.init();

    event_loop
        .run(move |event, control_flow| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
                    match event {
                        //Update and Render
                        WindowEvent::RedrawRequested => {
                            state.window().request_redraw();
                            state.core_update();
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    state.render_state.resize(state.render_state.size)
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    log::error!("OutOfMemory");
                                    control_flow.exit()
                                }
                                // This happaens when a frame takes too long to present
                                Err(wgpu::SurfaceError::Timeout) => {
                                    log::warn!("Surface timeout")
                                }
                            }
                        }

                        // Close / Exit
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                                    ..
                                },
                            ..
                        } => {
                            control_flow.exit();
                        }

                        // Reszie
                        WindowEvent::Resized(physical_size) => {
                            state.render_state.resize(*physical_size);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        })
        .unwrap();
}

struct State<'a> {
    render_state: RenderState<'a>,

    materials: Assets<Material>,
    material_instances: Assets<MaterialInstance>,
    meshes: Assets<UploadedMesh>,
    images: Assets<UploadedImage>,
    renderables: Vec<Arc<dyn DrawAble>>,

    depth_texture: UploadedImage,

    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group_layout: Arc<BindGroupLayout>,
}

struct RenderState<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,
}

impl<'a> State<'a> {
    async fn new(window: &'a Window) -> State<'a> {
        let render_state = RenderState::new(window).await;

        let camera =
            Camera::new(render_state.config.width as f32 / render_state.config.height as f32);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer =
            render_state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        let camera_bind_group_layout = Arc::new(
            render_state
                .device
                .create_bind_group_layout(&CameraUniform::layout_desc()),
        );

        let depth_texture = render_state.create_depth_texture();

        Self {
            render_state,

            materials: Assets::new(),
            material_instances: Assets::new(),
            meshes: Assets::new(),
            images: Assets::new(),
            renderables: vec![],

            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group_layout,

            depth_texture,
        }
    }

    pub fn init(&mut self) {
        self.load_default_material();

        let model = render::Model::load(
            AssetPath::Assets("Patagiosites laevis.glb".to_string()),
            &mut self.render_state,
        )
        .unwrap();

        let mut render = model
            .meshes
            .iter()
            .map(|it| Arc::new(it.upload(self)) as Arc<dyn DrawAble>)
            .collect::<Vec<_>>();
        self.renderables.append(&mut render);
    }

    fn draw_objects<'b>(&mut self, render_pass: &'b mut RenderPass<'b>) {
        let default_material = self
            .material_instances
            .get_by_name("default")
            .unwrap()
            .clone();
        let mut ctx = DrawContext {
            render_pass,
            default_material,
        };
        for renderable in self.renderables.iter() {
            renderable.draw(&mut ctx);
        }
    }

    pub fn load_default_material(&mut self) {
        let image = UploadedImage::load(
            AssetPath::Assets("@7ife_l-0.jpg".to_string()),
            &mut self.render_state,
        )
        .unwrap();

        let material = Arc::new(material_creations::unlit_textured_material(self));
        self.materials.insert_with_name("default", material.clone());

        let binding_groups = material.create_bind_groups(
            &self.render_state.device,
            vec![
                vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&image.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&image.sampler),
                    },
                ],
                vec![BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                }],
            ],
        );
        let material_instance = Arc::new(MaterialInstance {
            material: material.clone(),
            bind_groups: binding_groups,
        });
        self.material_instances
            .insert_with_name("default", material_instance);
    }

    pub fn window(&self) -> &Window {
        &self.render_state.window
    }

    fn reszie(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        todo!()
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        input::INPUT.lock().unwrap().update(event);
        false
    }

    fn core_update(&mut self) {
        TIME.lock().unwrap().update();
    }

    fn update(&mut self) {
        let time = TIME.lock().unwrap();
        let input = INPUT.lock().unwrap();

        let mut move_vec = Vector3::new(0., 0., 0.);
        if input.is_key_hold(KeyCode::KeyW) {
            move_vec += Vector3::new(0.0, 0.0, -1.0);
        }
        if input.is_key_hold(KeyCode::KeyA) {
            move_vec += Vector3::new(-1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::KeyS) {
            move_vec += Vector3::new(0.0, 0.0, 1.0);
        }
        if input.is_key_hold(KeyCode::KeyD) {
            move_vec += Vector3::new(1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::Space) {
            if input.is_key_hold(KeyCode::ShiftLeft) {
                move_vec += Vector3::new(0.0, -1.0, 0.0);
            } else {
                move_vec += Vector3::new(0.0, 1.0, 1.0);
            }
        }
        if move_vec != Vector3::new(0., 0., 0.) {
            move_vec = move_vec.normalize() * 0.5 * time.delta_time.as_secs_f32();
            self.camera.eye += move_vec;
            self.camera.target += move_vec;

            self.camera_uniform.update_view_proj(&self.camera);
            self.render_state.queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::cast_slice(&[self.camera_uniform]),
            );
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.render_state.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        {
            // 1. Render Pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    view: &self.depth_texture.view,
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.draw_objects(&mut render_pass);
        }

        self.render_state
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

impl<'a> RenderState<'a> {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(window: &'a Window) -> RenderState<'a> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();
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
            width: size.width,
            height: size.height,
            // determine how to sync
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
        }
    }

    fn create_depth_texture(&self) -> UploadedImage {
        let size = self.get_window_extend3d();
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
        let texture = self.device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
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

    fn get_window_extend3d(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.config.width.max(1),
            height: self.config.height.max(1),
            depth_or_array_layers: 1,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.5, 0.0],
        color: [1.0, 0.0, 0.0, 1.0],
        normal: [0.0, 0.0, 0.0],
        tex_coord: [0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5, 0.0],
        color: [0.0, 1.0, 0.0, 1.0],
        normal: [0.0, 0.0, 0.0],
        tex_coord: [0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5, 0.0],
        color: [0.0, 0.0, 1.0, 1.0],
        normal: [0.0, 0.0, 0.0],
        tex_coord: [1.0, 0.0],
    },
];

const INDICES: &[u32] = &[0, 1, 2];
