use std::sync::Arc;

use wgpu::{
    util::DeviceExt, BindGroup, BindGroupEntry, BindGroupLayout, Buffer, PipelineLayout,
    RenderPass, RenderPipeline, Sampler, Texture, TextureView,
};

pub mod camera;
pub mod material_creations;

pub struct DrawContext<'a> {
    pub render_pass: &'a mut RenderPass<'a>,
    pub default_material: Arc<MaterialInstance>,
}

pub trait DrawAble {
    fn draw(&self, context: &mut DrawContext);
}

impl DrawAble for UploadedMesh {
    fn draw(&self, context: &mut DrawContext) {
        let default_material = &context.default_material;
        let render_pass = &mut context.render_pass;
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for primitive in self.primitives.iter() {
            let material_instance = match &primitive.material_instance {
                Some(arc) => arc.clone(),
                None => default_material.clone(),
            };
            render_pass.set_pipeline(&material_instance.material.pipeline);
            for (i, bind_group) in material_instance.bind_groups.iter().enumerate() {
                render_pass.set_bind_group(i as u32, bind_group, &[]);
            }
            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }
}

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

pub struct UploadedMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub primitives: Vec<UploadedPrimitive>,
}

pub struct UploadedPrimitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material_instance: Option<Arc<MaterialInstance>>,
}

pub struct Renderable {
    pub mesh: Arc<UploadedMesh>,
}

pub struct Image {
    pub size: wgpu::Extent3d,
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
}

pub struct GltfMaterial {}

pub struct Model {
    pub meshes: Vec<Mesh>,
}

pub struct Primitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material: Option<GltfMaterial>,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub primitives: Vec<Primitive>,
}

impl Mesh {
    pub fn upload(&self, device: &wgpu::Device) -> UploadedMesh {
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
                material_instance: None,
            })
            .collect::<Vec<_>>();

        UploadedMesh {
            vertex_buffer,
            index_buffer,
            primitives,
        }
    }
}
