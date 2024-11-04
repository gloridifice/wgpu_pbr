use std::sync::Arc;

use asset::{load::Loadable, AssetPath, Assets};
use cgmath::{InnerSpace, Vector3};
use input::INPUT;
use render::{
    camera::{Camera, CameraUniform},
    material_creations, Image, Material, MaterialInstance, Renderable, UploadedMesh, Vertex,
};
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
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    state.resize(state.size)
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
                            state.resize(*physical_size);
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
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,

    materials: Assets<Material>,
    material_instances: Assets<MaterialInstance>,
    meshes: Assets<UploadedMesh>,
    images: Assets<Image>,
    renderables: Vec<Renderable>,

    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group_layout: Arc<BindGroupLayout>,
}

impl<'a> State<'a> {
    async fn new(window: &'a Window) -> State<'a> {
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

        let camera = Camera::new(config.width as f32 / config.height as f32);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&CameraUniform::layout_desc()));

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,

            materials: Assets::new(),
            material_instances: Assets::new(),
            meshes: Assets::new(),
            images: Assets::new(),
            renderables: vec![],

            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group_layout,
        }
    }

    pub fn init(&mut self) {
        // Render Pipeline

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(VERTICES),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let image = Image::load(AssetPath::Assets("@7ife_l-0.jpg".to_string()), self).unwrap();

        let material = Arc::new(material_creations::unlit_textured_material(self));
        let binding_groups = material.create_bind_groups(
            &self.device,
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

        let mesh = Arc::new(UploadedMesh {
            vertex_buffer,
            index_buffer,
        });

        let renderable = Renderable {
            mesh,
            material: material_instance.clone(),
            indices_num: INDICES.len() as u32,
            indices_start: 0,
        };
        self.renderables.push(renderable);
    }

    fn draw_objects(&mut self, render_pass: &mut RenderPass) {
        for renderable in self.renderables.iter() {
            let mesh = renderable.mesh.clone();
            let material_instance = renderable.material.clone();
            render_pass.set_pipeline(&material_instance.material.pipeline);
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for (i, bind_group) in material_instance.bind_groups.iter().enumerate() {
                render_pass.set_bind_group(i as u32, bind_group, &[]);
            }
            let start = renderable.indices_start;
            let num = renderable.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn reszie(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        todo!()
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        input::INPUT.lock().unwrap().update(event);
        false
    }

    fn update(&mut self) {
        let input = INPUT.lock().unwrap();
        let mut move_vec = Vector3::new(0., 0., 0.);
        if input.is_key_hold(KeyCode::KeyW) {
            move_vec += Vector3::new(0.0, 0.0, 1.0);
        }
        if input.is_key_hold(KeyCode::KeyA) {
            move_vec += Vector3::new(-1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::KeyS) {
            move_vec += Vector3::new(0.0, 0.0, -1.0);
        }
        if input.is_key_hold(KeyCode::KeyD) {
            move_vec += Vector3::new(1.0, 0.0, 0.0);
        }
        if input.is_key_hold(KeyCode::ShiftLeft) {
            move_vec += Vector3::new(0.0, -1.0, 0.0);
        }
        if input.is_key_hold(KeyCode::Space) {
            move_vec += Vector3::new(0.0, 1.0, 1.0);
        }
        if input.is_key_down(KeyCode::KeyJ) {
            println!("114514");
        }
        if move_vec != Vector3::new(0., 0., 0.) {
            move_vec = move_vec.normalize();
            self.camera.eye += move_vec * 0.1;
            self.camera.target += move_vec * 0.1;

            self.camera_uniform.update_view_proj(&self.camera);
            self.queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::cast_slice(&[self.camera_uniform]),
            );
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
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
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.draw_objects(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
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
