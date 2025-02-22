use std::sync::Arc;

use bevy_ecs::{
    system::Resource,
    world::{FromWorld, World},
};
use wgpu::{
    BindGroup, BindGroupLayout, BindingResource, PipelineLayout, RenderPipeline, ShaderStages,
    TextureViewDescriptor,
};
use write_g_buffer_pipeline::GBufferTexturesBindGroup;

use crate::{
    asset::{load::Loadable, AssetPath},
    bg_descriptor, bg_layout_descriptor,
    macro_utils::BGLEntry,
    wgpu_init, RenderState,
};

use super::{
    camera::CameraBuffer,
    cubemap::{CubeMapConverter, CubeMapConverterRgba8unorm, CubeVerticesBuffer},
    dfg::DFGTexture,
    light::{DynamicLightBindGroup, LightUnifromBuffer},
    shader_loader::ShaderLoader,
    shadow_mapping::ShadowMap,
    FullScreenVertexShader, UploadedImageWithSampler,
};

pub mod write_g_buffer_pipeline;

#[derive(Resource)]
pub struct GlobalBindGroup {
    pub bind_group: Arc<BindGroup>,
    pub layout: Arc<BindGroupLayout>,
}

#[allow(unused)]
#[derive(Resource)]
pub struct MainPipeline {
    pub pipeline: Arc<RenderPipeline>,
    pub pipeline_layout: Arc<PipelineLayout>,
    pub bind_group_layouts: Vec<Arc<BindGroupLayout>>,
}

impl FromWorld for GlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
        let hdri = UploadedImageWithSampler::load(
            AssetPath::Assets("textures/hdr/qwantani_afternoon_2k.hdr".to_string()),
            world,
        )
        .unwrap();

        let camera = world.resource::<CameraBuffer>();
        let light = world.resource::<LightUnifromBuffer>();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shadow_map = world.resource::<ShadowMap>();

        let bind_group_layout_desc = bg_layout_descriptor! {
            ["Main PBR Global Bind Group Layout"]
            0: ShaderStages::all() => BGLEntry::UniformBuffer(); // Camera
            1: ShaderStages::all() => BGLEntry::UniformBuffer(); // Light
            2: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Depth); // Light
            3: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Comparison); // Light
            4: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: false }); // Light
            5: ShaderStages::FRAGMENT => BGLEntry::TexCube(false, wgpu::TextureSampleType::Float { filterable: true }); // Light
            6: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering); // Light
        };

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let dfg = world.resource::<DFGTexture>();
        let cubemap = {
            let converter = world.resource::<CubeMapConverterRgba8unorm>();
            converter.0.render_hdir_to_cube_map(
                device,
                &hdri.view,
                &world.resource::<CubeVerticesBuffer>().vertices_buffer,
                512,
            )
        };
        let view = cubemap.create_view(&TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let bind_group_desc = bg_descriptor! {
            ["Main PBR Global BindGroup"][&layout]
            0: camera.buffer.as_entire_binding();
            1: light.buffer.as_entire_binding();
            2: BindingResource::TextureView(&shadow_map.image.view);
            3: BindingResource::Sampler(&shadow_map.image.sampler);
            4: BindingResource::TextureView(&dfg.texture.view);
            5: BindingResource::TextureView(&view);
            6: BindingResource::Sampler(&dfg.texture.sampler); // todo cubemap sampler
        };

        let bind_group = Arc::new(device.create_bind_group(&bind_group_desc));

        Self { bind_group, layout }
    }
}

impl FromWorld for MainPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let shader_source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("pbr_main"))
            .unwrap();
        let rs = &world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PBR Main"),
            source: shader_source,
        });
        let full_screen_shader = world.resource::<FullScreenVertexShader>();

        let bind_group_layouts = vec![
            Arc::clone(&world.resource::<GlobalBindGroup>().layout),
            Arc::clone(&world.resource::<GBufferTexturesBindGroup>().layout),
            Arc::clone(&world.resource::<DynamicLightBindGroup>().layout),
        ];

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("PBR Main Pipeline"),
                bind_group_layouts: &bind_group_layouts
                    .iter()
                    .map(|it| it.as_ref())
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu_init::full_screen_pipeline_desc(
            Some("PBR Main Pipeline"),
            &render_pipeline_layout,
            &full_screen_shader.module,
            &shader,
            &[Some(wgpu_init::color_target_replace_write_all(
                rs.config.format,
            ))],
        ));

        MainPipeline {
            pipeline: Arc::new(render_pipeline),
            pipeline_layout: Arc::new(render_pipeline_layout),
            bind_group_layouts,
        }
    }
}
