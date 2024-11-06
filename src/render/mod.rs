use std::sync::Arc;

use bevy_ecs::component::Component;
use cgmath::Matrix4;
use material_impl::{DefaultMaterial, DefaultMaterialInstance};
use transform::{Transform, TransformUniform};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, Buffer, PipelineLayout, RenderPass,
    RenderPipeline, Sampler, Texture, TextureView,
};

use crate::{App, State};

pub mod camera;
pub mod material_impl;
pub mod transform;

pub struct DrawContext<'a> {
    pub render_pass: &'a mut RenderPass<'a>,
    pub default_material: Arc<DefaultMaterialInstance>,
}

pub trait DrawAble {
    fn draw(&self, context: &mut DrawContext);
}

#[derive(Component)]
pub struct MeshRenderer {
    pub mesh: Option<Arc<UploadedMesh>>,
}

impl MeshRenderer {
    pub fn new(mesh: Arc<UploadedMesh>) -> Self {
        Self { mesh: Some(mesh) }
    }
}

pub struct Node {
    pub parent: Option<Arc<Node>>,
    pub children: Vec<Arc<Node>>,
    pub transform: Transform,
    pub mesh: Option<Arc<dyn DrawAble>>,
}

impl DrawAble for UploadedMesh {
    fn draw(&self, context: &mut DrawContext) {
        // context.state.render_state.queue.write_buffer(
        //     &context.state.transform_buffer,
        //     0,
        //     bytemuck::cast_slice(&[TransformUniform {
        //         matrix: self.transform_matrix().into(),
        //     }]),
        // );
        let default_material = &context.default_material;
        let render_pass = &mut context.render_pass;
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for primitive in self.primitives.iter() {
            let material_instance = match &primitive.material_instance {
                Some(arc) => arc.clone(),
                None => default_material.clone(),
            };
            render_pass.set_pipeline(&material_instance.material.pipeline());
            for (i, bind_group) in material_instance.bind_groups().iter().enumerate() {
                render_pass.set_bind_group(i as u32, bind_group, &[]);
            }
            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }

    // fn transform_matrix(&self) -> Matrix4<f32> {
    //     let matrix = self.transform.calculate_matrix4x4();
    //     match self.parent.as_ref() {
    //         Some(p) => matrix * p.transform_matrix(),
    //         None => matrix,
    //     }
    // }
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

pub trait MaterialPipeline: Sized {
    fn pipeline(&self) -> &RenderPipeline;
    fn pipeline_layout(&self) -> &PipelineLayout;
    fn bind_group_layouts(&self) -> &Vec<Arc<BindGroupLayout>>;
}

pub trait MaterialInstance<Parent>: Sized
where
    Parent: MaterialPipeline,
{
    fn parent(&self) -> Arc<Parent>;
    fn bind_groups(&self) -> Vec<Arc<BindGroup>>;
}

pub struct UploadedMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub primitives: Vec<UploadedPrimitive>,
}

pub struct UploadedPrimitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material_instance: Option<Arc<DefaultMaterialInstance>>,
}

pub struct UploadedImage {
    pub size: wgpu::Extent3d,
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
}

pub struct GltfMaterial {
    pub base_color_texture: Arc<UploadedImage>,
}

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
    pub fn upload(&self, state: &State) -> UploadedMesh {
        let device = &state.render_state.device;

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

        let default_material = state.materials.get_by_name("default").unwrap();

        let primitives = self
            .primitives
            .iter()
            .map(|it| UploadedPrimitive {
                indices_start: it.indices_start,
                indices_num: it.indices_num,
                material_instance: {
                    it.material.as_ref().map(|gltf_mat| {
                        Arc::new(DefaultMaterial::create_instance(
                            state,
                            Arc::clone(&default_material),
                            &gltf_mat.base_color_texture,
                        ))
                    })
                },
            })
            .collect::<Vec<_>>();

        UploadedMesh {
            vertex_buffer,
            index_buffer,
            primitives,
        }
    }
}

impl UploadedImage {
    pub fn image_data_layout(
        width: u32,
        heigh: u32,
        pixel_size: u32,
        offset: u64,
    ) -> wgpu::ImageDataLayout {
        wgpu::ImageDataLayout {
            offset,
            bytes_per_row: Some(pixel_size * width),
            rows_per_image: Some(heigh),
        }
    }

    pub fn default_sampler_desc() -> wgpu::SamplerDescriptor<'static> {
        wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }
    }

    pub fn from_glb_data(
        data: &gltf::image::Data,
        gltf_sampler: &gltf::texture::Sampler,
        state: &State,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };

        let texture = state
            .render_state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        let pixels = match data.format {
            gltf::image::Format::R8G8B8 => {
                let new_len = data.pixels.len() / 3 * 4;
                let mut ret = vec![0u8; new_len];
                for i in 0..new_len {
                    let divide = i / 4;
                    let modulo = i % 4;
                    ret[i] = if modulo != 3 {
                        *data.pixels.get(divide * 3 + modulo).unwrap()
                    } else {
                        0u8
                    };
                }
                ret
            }
            _ => data.pixels.clone(),
        };

        state.render_state.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            UploadedImage::image_data_layout(data.width, data.height, 4, 0),
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // todo
        let sampler = state
            .render_state
            .device
            .create_sampler(&UploadedImage::default_sampler_desc());

        Self {
            size,
            texture,
            view,
            sampler,
        }
    }
}
