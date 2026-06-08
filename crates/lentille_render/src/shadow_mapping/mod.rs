use crate::{app_ext::AppExt, camera, prelude::*, stage::RenderContext};
use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use cgmath::ortho;
use wgpu::{TextureView, TextureViewDimension, wgt::TextureDescriptor};

use super::{light::LightUnifromBuffer, shader_loader::ShaderLoader};

pub(crate) struct ShadowMappingPlugin;

impl Plugin for ShadowMappingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<ShadowMap>()
            .init_render_resource_with_config::<ShadowMapGlobalBindGroup>([after::<
                LightUnifromBuffer,
            >()])
            .init_render_resource_with_config::<ShadowMappingPipeline>([
                after::<ShadowMapGlobalBindGroup>(),
                after::<ObjectBindGroupLayout>(),
            ])
            .init_render_resource_with_config::<CascadeShadowMapping>([after::<
                ObjectBindGroupLayout,
            >()]);
    }
}

#[derive(Resource)]
pub struct ShadowMap {
    // For shadow map rendering pass
    pub image: UploadedImageWithSampler,
}

#[derive(Resource)]
pub struct ShadowMapGlobalBindGroup {
    pub layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct ShadowMappingPipeline {
    pub pipeline: Arc<RenderPipeline>,
    #[allow(unused)]
    pub layout: Arc<PipelineLayout>,
}

#[derive(Component, Clone, Default)]
pub struct CastShadow;

impl FromWorld for ShadowMapGlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        world.resource_scope(|world, render_state: Mut<RenderState>| {
            let device = &render_state.device;

            let layout = Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                ["Shadow Mapping Global Bind Group Layout"]
                0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            )));

            let light_uniform_buffer = &world.resource::<LightUnifromBuffer>().buffer;

            let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["Shadow Mapping Global Bind Group"] [ &layout ]
                0: light_uniform_buffer.as_entire_binding();
            )));

            Self { layout, bind_group }
        })
    }
}

impl FromWorld for ShadowMappingPipeline {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("light_depth_map"))
            .unwrap();
        let render_state = world.resource::<RenderState>();
        let device = &render_state.device;
        let global_bg_layout = world.resource::<ShadowMapGlobalBindGroup>();
        let object_bg_layout = world.resource::<ObjectBindGroupLayout>();

        let layout = Arc::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow mapping pipeline"),
                bind_group_layouts: &[Some(&global_bg_layout.layout), Some(&object_bg_layout.0)],
                immediate_size: 0,
            }),
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Mapping Light Shader"),
            source: shader_source,
        });

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Mapping Pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Vertex::desc()],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: RenderState::DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 4,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            }),
        );

        Self { pipeline, layout }
    }
}

impl FromWorld for ShadowMap {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let UploadedImage { texture, view } = camera::create_depth_texture(4096, 4096, device);
        let sampler = camera::create_depth_sampler(Some(wgpu::CompareFunction::LessEqual), device);

        Self {
            image: UploadedImageWithSampler {
                texture,
                view,
                sampler,
            },
        }
    }
}

pub struct LayerContext {
    pub view: Arc<TextureView>,
    pub mat_buffer: Arc<Buffer>,
    pub mat_bind_group: Arc<BindGroup>,
}

#[derive(Resource)]
pub struct CascadeShadowMapping {
    pub levels: usize,
    pub pipeline: Arc<RenderPipeline>,
    pub shadow_maps: Arc<wgpu::Texture>,
    pub layers: Vec<LayerContext>,
}

impl FromWorld for CascadeShadowMapping {
    fn from_world(world: &mut World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("cascade_shadow_mapping"))
            .unwrap();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let levels = 4;

        let texture_array = Arc::new(device.create_texture(&TextureDescriptor {
            label: Some("CSM Texture Array"),
            size: Extent3d {
                width: 2048,
                height: 2048,
                depth_or_array_layers: levels as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        }));

        let mut layers = Vec::new();

        let csm_uniform_layout =
            Arc::new(device.create_bind_group_layout(&bg_layout_descriptor! (
                ["Shadow Mapping Global Bind Group Layout"]
                0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            )));

        for i in 0..levels {
            let mat_buffer = Arc::new(device.create_buffer(&wgpu::wgt::BufferDescriptor {
                label: Some("ShadowMapping"),
                size: size_of::<[[f32; 4]; 4]>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            let mat_bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
                ["Shadow Mapping Global Bind Group"] [ csm_uniform_layout.as_ref() ]
                0: mat_buffer.as_entire_binding();
            )));

            let view = Arc::new(texture_array.create_view(&wgpu::TextureViewDescriptor {
                label: Some("CSM View Layer"),
                dimension: Some(TextureViewDimension::D2),
                format: Some(TextureFormat::Depth32Float),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            }));

            layers.push(LayerContext {
                view,
                mat_buffer,
                mat_bind_group,
            });
        }

        let object_bg_layout = world.resource::<ObjectBindGroupLayout>();

        let pipeline_layout = Arc::new(device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow mapping pipeline"),
                bind_group_layouts: &[Some(&csm_uniform_layout), Some(&object_bg_layout.0)],
                immediate_size: 0,
            },
        ));

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Mapping Light Shader"),
            source: shader_source,
        });

        let pipeline = Arc::new(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Shadow Mapping Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Vertex::desc()],
                },
                fragment: None,
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: RenderState::DEPTH_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 4,
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview_mask: None,
                cache: None,
            }),
        );

        Self {
            levels,
            pipeline,
            shadow_maps: texture_array,
            layers,
        }
    }
}

pub struct UpdateCsmBuffersCmd {
    pub light_dir: Vec3,
    pub inv_cam_view_proj: Mat4,
}

impl Command for UpdateCsmBuffersCmd {
    fn apply(self, world: &mut World) -> () {
        let rs = world.resource::<RenderState>();
        let csm = world.resource::<CascadeShadowMapping>();
        for i in 0..csm.levels {
            let slice = 1.0 / csm.levels as f32;
            let near = (i as f32) * slice;
            let far = near + slice;

            let mat4: [[f32; 4]; 4] =
                calculate_cascade_matrix(self.light_dir, self.inv_cam_view_proj, near, far).into();
            rs.queue
                .write_buffer(&csm.layers[i].mat_buffer, 0, bytemuck::bytes_of(&mat4));
        }
    }
}

fn calculate_cascade_matrix(light_dir: Vec3, inv: Mat4, near: f32, far: f32) -> Mat4 {
    let mut corners = Vec::with_capacity(8);
    let mut center = Vec3::zero();

    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let z = if z == 0 { near } else { far };
                let world_pt = inv * Vec4::new(2.0 * x as f32 - 1.0, 2.0 * y as f32 - 1.0, z, 1.0);
                center += Vec3::new(world_pt.x, world_pt.y, world_pt.z) / world_pt.w;
                corners.push(world_pt);
            }
        }
    }
    center /= 8.0;

    let view = Mat4::look_at_rh(
        (center + light_dir.normalize()).into_point(),
        center.into_point(),
        Vec3::unit_y(),
    );

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for &world_pt in &corners {
        let pt = view * world_pt;
        min_x = pt.x.min(min_x);
        max_x = pt.x.max(max_x);
        min_y = pt.y.min(min_y);
        max_y = pt.y.max(max_y);
        min_z = pt.z.min(min_z);
        max_z = pt.z.max(max_z);
    }

    let z_mult = 10.0;
    // min 向负方向扩大
    // max 向正的方向扩大
    min_z = if min_z < 0.0 {
        min_z * z_mult
    } else {
        min_z / z_mult
    };
    max_z = if max_z < 0.0 {
        max_z / z_mult
    } else {
        max_z * z_mult
    };
    let light_proj = ortho(min_x, max_x, min_y, max_y, min_z, max_z);

    return light_proj * view;
}
