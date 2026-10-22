use crate::prelude::*;
use wgpu::util::DeviceExt;
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
    pub images: Vec<gltf::image::Data>,
    pub materials: Vec<PbrMaterial>,
}
#[derive(Clone)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub primitives: Vec<Primitive>,
}
#[derive(Clone)]
pub struct Primitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material: Option<PbrMaterial>,
}

impl Mesh {
    pub fn upload(&self, device: &wgpu::Device) -> UploadedMesh {
        UploadedMesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh vertices"),
                contents: bytemuck::cast_slice(&self.vertices),
                usage: BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh indices"),
                contents: bytemuck::cast_slice(&self.indices),
                usage: BufferUsages::INDEX,
            }),
            primitives: self
                .primitives
                .iter()
                .map(|p| UploadedPrimitive {
                    indices_start: p.indices_start,
                    indices_num: p.indices_num,
                })
                .collect(),
        }
    }
    pub fn plane(size: f32) -> Self {
        let h = size * 0.5;
        let vertices = [
            ([-h, 0., -h], [0., 0.]),
            ([h, 0., -h], [1., 0.]),
            ([h, 0., h], [1., 1.]),
            ([-h, 0., h], [0., 1.]),
        ]
        .into_iter()
        .map(|(position, tex_coord)| Vertex {
            position,
            tex_coord,
            normal: [0., 1., 0.],
            tangent: [1., 0., 0.],
            color: [1.; 4],
        })
        .collect();
        Self {
            vertices,
            indices: vec![2, 1, 0, 3, 2, 0],
            primitives: vec![Primitive {
                indices_start: 0,
                indices_num: 6,
                material: None,
            }],
        }
    }
}
impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x4, 4 => Float32x2];
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
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
}
