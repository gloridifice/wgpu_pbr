use std::sync::Arc;

use wgpu::{include_wgsl, ShaderStages};

use crate::{bg_layout_descriptor, impl_pod_zeroable, macro_utils::BGLEntry, math_type::Vec4};

use super::{defered_rendering::MainGlobalBindGroup, prelude::*};

#[derive(Component)]
pub struct GizmosMesh {
    color: Vec4,
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
    pub material_bind_group_layout: Arc<BindGroupLayout>,
}

// impl FromWorld for GizmosPipeline {
//     fn from_world(world: &mut World) -> Self {
//         let rs = world.resource::<RenderState>();
//         let device = &rs.device;

//         let global = world.resource::<MainGlobalBindGroup>();
//         let model = world.resource::<ObjectBindGroupLayout>();

//         let bg_layout_desc = bg_layout_descriptor! {
//             ["Gizmos"]
//             0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer();
//         };
//         let bg_layout = device.create_bind_group_layout(&bg_layout_desc);

//         let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
//             label: Some("Gizmos"),
//             bind_group_layouts: &[&global.layout, &bg_layout, &model.0],
//             push_constant_ranges: &[],
//         });

//         let shader = device.create_shader_module(include_wgsl!("../../assets/shaders/gizmos.wgsl"));

//         let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
//             label: Some("Gizmos"),
//             layout: Some(&pipeline_layout),
//             vertex: wgpu_init::vertex_state(&shader, &[Vertex::desc()]),
//             primitive: wgpu_init::primitive_triangle_list_default(),
//             depth_stencil: (),
//             multisample: (),
//             fragment: (),
//             multiview: (),
//             cache: (),
//         });
//     }
// }

pub fn sys_render_gizmos() {}
