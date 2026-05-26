use std::sync::Arc;

use wgpu::util::DeviceExt;

use crate::material::pbr::UploadedPBRMaterial;

use super::{
    NormalDefaultTexture, WhiteTexture, deferred_rendering::DeferredComputePipeline, prelude::*,
};
use bevy_ecs::prelude::*;

pub mod renderer;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub color: [f32; 4],
    pub tex_coord: [f32; 2],
}

impl_pod_zeroable!(Vertex);

pub struct Model {
    pub meshes: Vec<Mesh>,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub primitives: Vec<Primitive>,
}

pub struct Primitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material: Option<GltfMaterial>,
}

impl Mesh {
    pub fn upload(&self, world: &World) -> UploadedMesh {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let main_pipeline = world.resource::<DeferredComputePipeline>();
        let layout = world.resource::<PBRMaterialBindGroupLayout>();
        let white_tex = world.resource::<WhiteTexture>();
        let normal_default = world.resource::<NormalDefaultTexture>();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&self.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let primitives = self
            .primitives
            .iter()
            .map(|it| UploadedPrimitive {
                indices_start: it.indices_start,
                indices_num: it.indices_num,
                uploaded_material: {
                    it.material.as_ref().map(|gltf_mat| {
                        Arc::new(UploadedPBRMaterial::from_gltf(
                            device,
                            layout,
                            &white_tex.0,
                            &normal_default.0,
                            Arc::clone(&main_pipeline.pipeline),
                            gltf_mat,
                        ))
                    })
                },
                material: it.material.as_ref().map(|it| Arc::new(it.clone())),
            })
            .collect::<Vec<_>>();

        UploadedMesh {
            vertex_buffer,
            index_buffer,
            primitives,
        }
    }
}

impl Vertex {
    #[rustfmt::skip]
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3, // Position
        1 => Float32x3, // Normal
        2 => Float32x3, // Tangent
        3 => Float32x4, // Color
        4 => Float32x2, // UV0
    ];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Vertex::ATTRIBS,
        }
    }
}

pub struct UploadedMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub primitives: Vec<UploadedPrimitive>,
}

pub struct UploadedPrimitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub uploaded_material: Option<Arc<UploadedPBRMaterial>>,
    pub material: Option<Arc<GltfMaterial>>,
}
