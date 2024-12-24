use std::sync::Arc;

use bevy_ecs::{
    component::Component,
    system::Resource,
    world::{FromWorld, Mut, World},
};
use camera::RenderCamera;
use light::RenderLight;
use pbr_pipeline::{Material, PBRMaterial};
use shadow_mapping::ShadowMap;
use transform::TransformUniform;
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, BindingResource, Buffer, BufferDescriptor,
    BufferUsages, Extent3d, RenderPass, Sampler, SamplerBindingType, ShaderStages, Texture,
    TextureDescriptor, TextureDimension, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor,
};

use crate::{
    asset::{load::Loadable, AssetPath},
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    RenderState, State,
};

pub mod camera;
pub mod light;
pub mod pbr_pipeline;
pub mod shadow_mapping;
pub mod transform;
pub mod post_processing;
pub mod systems;

#[derive(Resource)]
pub struct ColorRenderTarget(pub Option<UploadedImage>);
#[derive(Resource)]
pub struct DepthRenderTarget(pub Option<UploadedImage>);

#[derive(Resource)]
pub struct RenderTargetSize {
    pub width: u32,
    pub height: u32,
}

impl Default for RenderTargetSize {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
        }
    }
}

pub fn create_color_render_target_image(
    width: u32,
    height: u32,
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> UploadedImage {
    let size = Extent3d {
        width: width,
        height: height,
        depth_or_array_layers: 1,
    };
    let desc = TextureDescriptor {
        label: Some("Render Target"),
        size,
        format: config.format,
        usage: config.usage | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        view_formats: &[],
    };
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        // 4.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        compare: None, // 5.
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    UploadedImage {
        size,
        texture,
        view,
        sampler,
    }
}

pub fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> UploadedImage {
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let desc = wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RenderState::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[RenderState::DEPTH_FORMAT],
    };
    let texture = device.create_texture(&desc);
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Shadow Map"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual), // 5.
        ..Default::default()
    });

    UploadedImage {
        size,
        texture,
        view,
        sampler,
    }
}

impl FromWorld for ColorRenderTarget {
    fn from_world(world: &mut World) -> Self {
        let render_state = world.resource::<RenderState>();
        let size = world.resource::<RenderTargetSize>();

        let target = create_color_render_target_image(
            size.width,
            size.height,
            &render_state.device,
            &render_state.config,
        );

        Self(Some(target))
    }
}

impl FromWorld for DepthRenderTarget {
    fn from_world(world: &mut World) -> Self {
        let render_state = world.resource::<RenderState>();
        let size = world.resource::<RenderTargetSize>();

        let target = create_depth_texture(
            &render_state.device,
            size.width,
            size.height,
        );

        Self(Some(target))
    }
}

pub struct DrawContext<'a, 'b> {
    pub render_pass: &'b mut RenderPass<'a>,
    pub world: &'b World,
}

pub trait DrawAble {
    fn draw_depth(&self, render_pass: &mut RenderPass);

    fn draw_main(&self, render_pass: &mut RenderPass, default_material: Arc<PBRMaterial>);
}

#[derive(Component)]
pub struct MeshRenderer {
    pub mesh: Option<Arc<UploadedMesh>>,
    pub object_bind_group: Arc<BindGroup>,
    pub transform_buffer: Arc<Buffer>,
}

impl MeshRenderer {
    pub fn new(mesh: Arc<UploadedMesh>, world: &World) -> Self {
        let device = &world.resource::<RenderState>().device;
        let layout = &world.resource::<ObjectBindGroupLayout>().0;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("transform buffer"),
            size: size_of::<TransformUniform>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let object_bind_group = device.create_bind_group(&bg_descriptor!(
            ["Object Bind Group"] [layout]
            0: buffer.as_entire_binding();
        ));
        Self {
            mesh: Some(mesh),
            object_bind_group: Arc::new(object_bind_group),
            transform_buffer: Arc::new(buffer),
        }
    }

    pub fn update_transform_buffer(&self, queue: &wgpu::Queue, uniform: TransformUniform) {
        queue.write_buffer(&self.transform_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

impl DrawAble for MeshRenderer {
    fn draw_depth(&self, render_pass: &mut RenderPass) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };

        render_pass.set_bind_group(1, &self.object_bind_group, &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for primitive in mesh.primitives.iter() {
            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }
    fn draw_main(&self, render_pass: &mut RenderPass, default_material: Arc<PBRMaterial>) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };

        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(2, &self.object_bind_group, &[]);

        let mut last_material: Option<Arc<PBRMaterial>> = None;

        for primitive in mesh.primitives.iter() {
            let material_instance = match primitive.material_instance.as_ref() {
                Some(a) => a,
                None => &default_material,
            };

            if last_material.is_none()
                || Arc::ptr_eq(last_material.as_ref().unwrap(), material_instance)
            {
                last_material = Some(Arc::clone(&material_instance));
                render_pass.set_bind_group(1, material_instance.get_bind_group(), &[]);
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

pub struct UploadedMesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub primitives: Vec<UploadedPrimitive>,
}

pub struct UploadedPrimitive {
    pub indices_start: u32,
    pub indices_num: u32,
    pub material_instance: Option<Arc<PBRMaterial>>,
}

pub struct UploadedImage {
    #[allow(unused)]
    pub size: wgpu::Extent3d,
    #[allow(unused)]
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
        let device = &state.render_state().device;

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
                material_instance: {
                    it.material
                        .as_ref()
                        .map(|gltf_mat| Arc::new(PBRMaterial::form_gltf(&state.world, &gltf_mat)))
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
        #[allow(unused)] gltf_sampler: &gltf::texture::Sampler,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
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

        queue.write_texture(
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
        let sampler = device.create_sampler(&UploadedImage::default_sampler_desc());

        Self {
            size,
            texture,
            view,
            sampler,
        }
    }
}

#[derive(Resource, Clone)]
pub struct ObjectBindGroupLayout(Arc<BindGroupLayout>);

#[derive(Resource, Clone)]
pub struct MaterialBindGroupLayout(Arc<BindGroupLayout>);

#[derive(Resource, Clone)]
pub struct GlobalBindGroup {
    #[allow(unused)]
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

impl FromWorld for ObjectBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let object_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Object Bind Group Layout"]
                0: ShaderStages::VERTEX => BGLEntry::UniformBuffer(); // Transform
            )));
        Self(object_bind_group_layout)
    }
}

impl FromWorld for MaterialBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let material_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor!(
                ["Material Bind Group Layout"]
                0: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true });
                1: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
            )));
        Self(material_bind_group_layout)
    }
}

impl FromWorld for GlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        world.resource_scope(|world, rs: Mut<RenderState>| {
            let device = &rs.device;

            let bind_group_layout =
                Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                    ["Global Bind Group Layout"]
                    0: ShaderStages::VERTEX => BGLEntry::UniformBuffer(); // Camera Uniform
                    1: ShaderStages::all() => BGLEntry::UniformBuffer(); // Global Light Uniform
                    2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Depth); // Shadow Map
                    3: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Comparison); // Shadow Map
                )));

            let camera_uniform_buffer = &world.resource::<RenderCamera>().buffer;
            let light_uniform_buffer = &world.resource::<RenderLight>().buffer;
            let shadow_map_image = &world.resource::<ShadowMap>().image;

            let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["Global Bind Group"] [ &bind_group_layout ]
                0: camera_uniform_buffer.as_entire_binding();
                1: light_uniform_buffer.as_entire_binding();
                2: BindingResource::TextureView(&shadow_map_image.view);
                3: BindingResource::Sampler(&shadow_map_image.sampler);
            )));

            GlobalBindGroup {
                layout: bind_group_layout,
                bind_group,
            }
        })
    }
}

#[derive(Resource, Clone)]
pub struct DefaultMainPipelineMaterial(Arc<PBRMaterial>);

impl FromWorld for DefaultMainPipelineMaterial {
    fn from_world(world: &mut World) -> Self {
        let image =
            UploadedImage::load(AssetPath::Assets("textures/default.png".to_string()), world).unwrap();

        let mat = PBRMaterial::form_gltf(
            world,
            &GltfMaterial {
                base_color_texture: Arc::new(image),
            },
        );
        Self(Arc::new(mat))
    }
}
