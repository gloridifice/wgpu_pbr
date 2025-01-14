use std::sync::Arc;

use cgmath::{ortho, Matrix4, Point3};
use wgpu::{include_wgsl, util::DeviceExt, BufferUsages, ShaderStages};

use crate::{
    bg_descriptor, bg_layout_descriptor, impl_pod_zeroable, macro_utils::BGLEntry, math_type::Vec4,
};

use super::{
    camera::OPENGL_TO_WGPU_MATRIX,
    create_depth_texture,
    material::{register_material_by_world, MaterialData, MaterialInstance},
    prelude::*,
    ColorRenderTarget,
};

#[derive(Component)]
pub struct Gizmos {
    pub instance: Arc<MaterialInstance<GizmosMaterial>>,
}

#[derive(Debug, Clone)]
pub struct GizmosMaterial {
    color: Vec4,
}

impl MaterialData for GizmosMaterial {
    type Raw = RawGizmosMaterial;

    fn raw(&self) -> Self::Raw {
        Self::Raw {
            color: self.color.into(),
        }
    }

    fn binding_resources<'a>(&self, buffer: &'a wgpu::Buffer) -> Vec<wgpu::BindingResource<'a>> {
        vec![buffer.as_entire_binding()]
    }
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct RawGizmosMaterial {
    color: [f32; 4],
}

impl_pod_zeroable!(RawGizmosMaterial);

#[derive(Resource, Clone)]
pub struct GizmosPipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub layout: Arc<PipelineLayout>,
    pub depth_texture: Arc<UploadedImageWithSampler>,
}

#[derive(Resource, Clone)]
pub struct GizmosGlobalBindGroup {
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct GizmosGlobalUniform {
    view_proj: [[f32; 4]; 4],
}

impl_pod_zeroable!(GizmosGlobalUniform);

impl GizmosMaterial {
    pub fn new(color: Vec4) -> Self {
        Self { color }
    }

    pub fn raw(&self) -> RawGizmosMaterial {
        RawGizmosMaterial {
            color: self.color.into(),
        }
    }
}

impl FromWorld for GizmosGlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let color_target = world.resource::<ColorRenderTarget>();

        let bg_layout_desc = bg_layout_descriptor! {
            ["Gizmos Global"]
            0: ShaderStages::VERTEX => BGLEntry::UniformBuffer();
        };
        let size = color_target.0.as_ref().unwrap().size;
        let right = 1.0f32;
        let top = right * size.height as f32 / size.width as f32;
        let view = Matrix4::look_at_rh(
            Point3::new(0., 0., 0.),
            Point3::new(0., 0., -1.),
            cgmath::Vector3::new(0., 1., 0.),
        );
        let view_porj = OPENGL_TO_WGPU_MATRIX * ortho(-right, right, -top, top, 0.1, 100.) * view;
        let uniform = GizmosGlobalUniform {
            view_proj: view_porj.into(),
        };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Gizomos Global"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let layout = Arc::new(device.create_bind_group_layout(&bg_layout_desc));
        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["Gizomos Global"][&layout]
                0: buffer.as_entire_binding();
        )));

        Self { layout, bind_group }
    }
}

impl FromWorld for GizmosPipeline {
    fn from_world(world: &mut World) -> Self {
        let bg_layout_desc = bg_layout_descriptor! {
            ["Gizmos"]
            0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer();
        };
        let bg_layout = register_material_by_world::<GizmosMaterial>(world, &bg_layout_desc);

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let global = world.resource::<GizmosGlobalBindGroup>();
        let model = world.resource::<ObjectBindGroupLayout>();

        let depth_texture = Arc::new(create_depth_texture(device, 256, 256, None));

        let layout = Arc::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Gizmos"),
                bind_group_layouts: &[&global.layout, &bg_layout, &model.0],
                push_constant_ranges: &[],
            }),
        );

        let shader = device.create_shader_module(include_wgsl!("../../assets/shaders/gizmos.wgsl"));

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Gizmos"),
                layout: Some(&layout),
                vertex: wgpu_init::vertex_state(&shader, &[Vertex::desc()]),
                primitive: wgpu_init::primitive_triangle_list_default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: RenderState::DEPTH_FORMAT,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: rs.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                multiview: None,
                cache: None,
            }),
        );
        Self {
            pipeline,
            layout,
            depth_texture,
        }
    }
}

impl GizmosPipeline {
    pub fn resize(&mut self, width: u32, height: u32, device: &wgpu::Device) {
        self.depth_texture = Arc::new(create_depth_texture(device, width, height, None));
    }
}
