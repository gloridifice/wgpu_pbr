use std::sync::{LazyLock, Mutex};

use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use wgpu::util::DeviceExt;

use crate::{
    SCREEN_FORMAT, app_ext::AppExt, camera::CameraBuffer, graph::after, prelude::*,
    shader_loader::ShaderLoader, stage::RenderContext,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GizmoLineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl GizmoLineVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x4,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<GizmoLineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

unsafe impl bytemuck::Pod for GizmoLineVertex {}
unsafe impl bytemuck::Zeroable for GizmoLineVertex {}

pub enum GizmoPrimitive {
    Line {
        start: Vec3,
        end: Vec3,
        color: Color,
    },
}

pub static GIZMO_BUFFER: LazyLock<Mutex<Vec<GizmoPrimitive>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

pub struct Gizmo;

impl Gizmo {
    pub fn line(start: Vec3, end: Vec3, color: Color) {
        GIZMO_BUFFER
            .lock()
            .unwrap()
            .push(GizmoPrimitive::Line { start, end, color });
    }

    pub fn dot(position: Vec3, radius: f32, color: Color) {
        let mut buf = GIZMO_BUFFER.lock().unwrap();
        let r = radius;
        buf.push(GizmoPrimitive::Line {
            start: Vec3::new(position.x - r, position.y, position.z),
            end: Vec3::new(position.x + r, position.y, position.z),
            color,
        });
        buf.push(GizmoPrimitive::Line {
            start: Vec3::new(position.x, position.y - r, position.z),
            end: Vec3::new(position.x, position.y + r, position.z),
            color,
        });
        buf.push(GizmoPrimitive::Line {
            start: Vec3::new(position.x, position.y, position.z - r),
            end: Vec3::new(position.x, position.y, position.z + r),
            color,
        });
    }

    pub fn r#box(a: Vec3, b: Vec3, color: Color) {
        let mut buf = GIZMO_BUFFER.lock().unwrap();
        let x0 = a.x.min(b.x);
        let x1 = a.x.max(b.x);
        let y0 = a.y.min(b.y);
        let y1 = a.y.max(b.y);
        let z0 = a.z.min(b.z);
        let z1 = a.z.max(b.z);

        let corners = [
            Vec3::new(x0, y0, z0),
            Vec3::new(x1, y0, z0),
            Vec3::new(x1, y1, z0),
            Vec3::new(x0, y1, z0),
            Vec3::new(x0, y0, z1),
            Vec3::new(x1, y0, z1),
            Vec3::new(x1, y1, z1),
            Vec3::new(x0, y1, z1),
        ];

        // bottom face (z0)
        for i in 0..4 {
            buf.push(GizmoPrimitive::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
                color,
            });
        }
        // top face (z1)
        for i in 0..4 {
            buf.push(GizmoPrimitive::Line {
                start: corners[4 + i],
                end: corners[4 + (i + 1) % 4],
                color,
            });
        }
        // vertical edges
        for i in 0..4 {
            buf.push(GizmoPrimitive::Line {
                start: corners[i],
                end: corners[4 + i],
                color,
            });
        }
    }
}

#[derive(Resource)]
pub struct GizmoBindGroupLayout(pub Arc<BindGroupLayout>);

impl FromWorld for GizmoBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let desc = bg_layout_descriptor! {
            ["Gizmo Bind Group Layout"]
            0: ShaderStages::VERTEX => BGLEntry::UniformBuffer();
        };

        Self(Arc::new(device.create_bind_group_layout(&desc)))
    }
}

#[derive(Resource)]
pub struct GizmoPipeline {
    pub pipeline: Arc<RenderPipeline>,
    #[allow(unused)]
    pub layout: Arc<PipelineLayout>,
}

impl FromWorld for GizmoPipeline {
    fn from_world(world: &mut World) -> Self {
        let mut shader = world.resource_mut::<ShaderLoader>();
        let shader_source = shader
            .load_source(AssetPath::new_shader_wgsl("gizmos"))
            .unwrap();

        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let gizmo_layout = world.resource::<GizmoBindGroupLayout>();

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: shader_source,
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Gizmo Pipeline Layout"),
            bind_group_layouts: &[Some(gizmo_layout.0.as_ref())],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gizmo Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[GizmoLineVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SCREEN_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: RenderState::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline: Arc::new(pipeline),
            layout: Arc::new(layout),
        }
    }
}

pub struct GizmoStage;

pub(crate) struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.configure_render_stage::<GizmoStage>([after::<crate::PostProcessStage>()])
            .add_render_system_with_config::<GizmoStage, _, _>(sys_render_gizmo_pass, [])
            .init_render_resource_with_config::<GizmoBindGroupLayout>([])
            .init_render_resource_with_config::<GizmoPipeline>([after::<GizmoBindGroupLayout>()]);
    }
}

pub fn sys_render_gizmo_pass(
    ctx: InMut<RenderContext>,
    gizmo_pipeline: Res<GizmoPipeline>,
    gizmo_layout: Res<GizmoBindGroupLayout>,
    q_camera_buffer: Query<&CameraBuffer>,
    rs: Res<RenderState>,
) {
    let InMut(RenderContext {
        camera_id,
        encoder,
        color_target,
        camera_global_bind_group: _,
        depth_target,
        gizmo_primitives,
    }) = ctx;

    let camera_buffer = match q_camera_buffer.get(*camera_id) {
        Ok(cb) => cb,
        Err(_) => return,
    };

    if gizmo_primitives.is_empty() {
        return;
    }

    let mut vertices: Vec<GizmoLineVertex> = Vec::with_capacity(gizmo_primitives.len() * 2);

    for p in gizmo_primitives.iter() {
        match p {
            GizmoPrimitive::Line { start, end, color } => {
                let c = color.into_array();
                vertices.push(GizmoLineVertex {
                    position: [start.x, start.y, start.z],
                    color: c,
                });
                vertices.push(GizmoLineVertex {
                    position: [end.x, end.y, end.z],
                    color: c,
                });
            }
        }
    }

    if vertices.is_empty() {
        return;
    }

    let device = &rs.device;

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Gizmo Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: BufferUsages::VERTEX,
    });

    let bind_group = device.create_bind_group(&bg_descriptor!(
        ["Gizmo BindGroup"] [&gizmo_layout.0]
        0: camera_buffer.buffer.as_entire_binding();
    ));

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Gizmo Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &color_target.view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_target.as_ref().map(|dt| {
            wgpu::RenderPassDepthStencilAttachment {
                view: &dt.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }
        }),
        ..Default::default()
    });

    render_pass.set_pipeline(&gizmo_pipeline.pipeline);
    render_pass.set_bind_group(0, Some(&bind_group), &[]);
    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    render_pass.draw(0..vertices.len() as u32, 0..1);
}
