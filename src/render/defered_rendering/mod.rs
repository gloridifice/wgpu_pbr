use std::sync::Arc;

use bevy_ecs::{
    system::Resource,
    world::{FromWorld, World},
};
use wgpu::{
    BindGroup, BindGroupLayout, BindingResource, PipelineLayout, RenderPipeline, ShaderStages,
};
use write_g_buffer_pipeline::GBufferTexturesBindGroup;

use crate::{
    asset::AssetPath, bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry, wgpu_init,
    RenderState,
};

use super::{
    camera::CameraBuffer,
    light::{DynamicLightBindGroup, LightUnifromBuffer},
    shader_loader::ShaderLoader,
    shadow_mapping::ShadowMap,
    FullScreenVertexShader,
};

pub mod write_g_buffer_pipeline;

#[derive(Resource)]
pub struct MainGlobalBindGroup {
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

impl FromWorld for MainGlobalBindGroup {
    fn from_world(world: &mut World) -> Self {
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
        };

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let bind_group_desc = bg_descriptor! {
            ["Main PBR Global BindGroup"][&layout]
            0: camera.buffer.as_entire_binding();
            1: light.buffer.as_entire_binding();
            2: BindingResource::TextureView(&shadow_map.image.view);
            3: BindingResource::Sampler(&shadow_map.image.sampler);
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
            Arc::clone(&world.resource::<MainGlobalBindGroup>().layout),
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
