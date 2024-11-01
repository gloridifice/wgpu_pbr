use std::sync::Arc;

use wgpu::{naga::Handle, BindGroup, Buffer, PipelineLayout, RenderPass, RenderPipeline};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub tex_coord: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Vertex::ATTRIBS,
        }
    }
}

pub struct Material {
    pub pipeline: RenderPipeline,
    pub layout: PipelineLayout,
}

pub struct MaterialInstance {
    pub material: Handle<Material>,
    pub bind_groups: &'static [BindGroup],
}

pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub indices_start: u32,
    pub indices_num: u32,
}

pub struct Renderable {
    pub mesh: Handle<Mesh>,
    pub material: Handle<MaterialInstance>,
}
