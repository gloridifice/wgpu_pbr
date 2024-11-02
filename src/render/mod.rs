use std::sync::Arc;

use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, Buffer, PipelineLayout, RenderPipeline, Sampler,
    Texture, TextureView,
};

pub mod camera;
pub mod material_creations;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub tex_coord: [f32; 2],
}

unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}

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
    pub pipeline_layout: PipelineLayout,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl Material {
    pub fn create_bind_groups(
        &self,
        device: &wgpu::Device,
        entries_vec: Vec<Vec<BindGroupEntry>>,
    ) -> Vec<Arc<BindGroup>> {
        let mut ret = vec![];
        for (i, entries) in entries_vec.iter().enumerate() {
            ret.push(Arc::new(device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    layout: self.bind_group_layouts.get(i).unwrap(),
                    entries,
                    label: None,
                },
            )));
        }
        ret
    }
}

pub struct MaterialInstance {
    pub material: Arc<Material>,
    pub bind_groups: Vec<Arc<BindGroup>>,
}

pub struct MeshSurface {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

pub struct Renderable {
    pub mesh: Arc<MeshSurface>,
    pub material: Arc<MaterialInstance>,
    pub indices_start: u32,
    pub indices_num: u32,
}

pub struct Image {
    pub size: wgpu::Extent3d,
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
}
