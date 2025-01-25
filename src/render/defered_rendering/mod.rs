use std::sync::Arc;

use bevy_ecs::{
    system::Resource,
    world::{FromWorld, World},
};
use wgpu::{
    util::DeviceExt, BindGroup, BindGroupLayout, BindingResource, BufferUsages, PipelineLayout,
    RenderPipeline, ShaderStages,
};
use write_g_buffer_pipeline::{GBufferTexturesBindGroup, RawPBRMaterial};

use crate::{bg_descriptor, bg_layout_descriptor, macro_utils::BGLEntry, wgpu_init, RenderState};

use super::{
    camera::CameraBuffer,
    light::{DynamicLightBindGroup, LightUnifromBuffer},
    material::UploadedMaterial,
    FullScreenVertexShader, GltfMaterial, PBRMaterialBindGroupLayout, UploadedImageWithSampler,
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

        let bind_group_layout_desc = bg_layout_descriptor! {
            ["Main PBR Pipeline"]
            0: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer(); // Camera
            1: ShaderStages::FRAGMENT => BGLEntry::UniformBuffer(); // Light
        };

        let layout = Arc::new(device.create_bind_group_layout(&bind_group_layout_desc));

        let bind_group_desc = bg_descriptor! {
            ["Main PBR BindGroup"][&layout]
            0: camera.buffer.as_entire_binding();
            1: light.buffer.as_entire_binding();
        };

        let bind_group = Arc::new(device.create_bind_group(&bind_group_desc));

        Self { bind_group, layout }
    }
}

impl FromWorld for MainPipeline {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<RenderState>();

        let device = &rs.device;
        let shader = device
            .create_shader_module(wgpu::include_wgsl!("../../../assets/shaders/pbr_main.wgsl"));
        let full_screen_shader = world.resource::<FullScreenVertexShader>();

        let bind_group_layouts = vec![
            Arc::clone(&world.resource::<GBufferTexturesBindGroup>().layout),
            Arc::clone(&world.resource::<MainGlobalBindGroup>().layout),
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

pub struct UploadedPBRMaterial {
    pub bind_group: Arc<BindGroup>,
    pub pipeline: Arc<RenderPipeline>,
}

impl UploadedPBRMaterial {
    pub fn from_gltf(
        device: &wgpu::Device,
        layout: &PBRMaterialBindGroupLayout,
        white_texture: &UploadedImageWithSampler,
        main_pipeline: Arc<RenderPipeline>,
        gltf_material: &GltfMaterial,
    ) -> Self {
        let base_color = gltf_material
            .base_color_texture
            .as_ref()
            .map(|it| it.as_ref())
            .unwrap_or(white_texture);
        let material_bind_group_layout = &layout.0;

        let raw = RawPBRMaterial::from(gltf_material);

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PBR"),
            contents: bytemuck::cast_slice(&[raw]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind_group = Arc::new(device.create_bind_group(&bg_descriptor!(
            ["PBR Material Bind Group"] [material_bind_group_layout]
            0: buffer.as_entire_binding();
            1: BindingResource::TextureView(&base_color.view);
            2: BindingResource::Sampler(&base_color.sampler);
        )));

        Self {
            bind_group,
            pipeline: main_pipeline,
        }
    }
}

impl UploadedMaterial for UploadedPBRMaterial {
    fn get_bind_group(&self) -> &BindGroup {
        &self.bind_group
    }
    fn get_render_pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}
