use std::{collections::HashMap, sync::Arc};

use bevy_ecs::prelude::*;
use wgpu::{
    BindGroup, BindGroupLayout, BindingResource, Device, PipelineLayout, PipelineLayoutDescriptor,
    RenderPipeline, ShaderModule, ShaderStages, SurfaceConfiguration,
};

use crate::{bg_descriptor, bg_layout_descriptor, render::BGLEntry, wgpu_init};

use super::{
    create_color_render_target_image, FullScreenVertexShader, RenderTargetSize,
    UploadedImageWithSampler,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RenderStage {
    BeforeOpaque,
    AfterOpaque,
    BeforeTransparent,
    AfterTransparent,
}

#[derive(Resource)]
pub struct PostProcessingManager {
    pub pipelines: HashMap<RenderStage, Vec<PostProcessingPipeline>>,
    pub pipeline_layout: Arc<PipelineLayout>,
    #[allow(unused)]
    pub bind_group_layout: Arc<BindGroupLayout>,
    bind_group_0: Arc<BindGroup>,
    bind_group_1: Arc<BindGroup>,
    temp_texture_0: Arc<UploadedImageWithSampler>,
    temp_texture_1: Arc<UploadedImageWithSampler>,
    temp_texture_index: usize,
    pub vs_shader: Arc<ShaderModule>,
}

impl PostProcessingManager {
    pub fn get_current_source_texture(&self) -> Arc<UploadedImageWithSampler> {
        match self.temp_texture_index {
            0 => Arc::clone(&self.temp_texture_0),
            _ => Arc::clone(&self.temp_texture_1),
        }
    }
    pub fn next_source_and_target(&mut self) -> (Arc<BindGroup>, Arc<UploadedImageWithSampler>) {
        let ret = match self.temp_texture_index {
            0 => (
                Arc::clone(&self.bind_group_0),
                Arc::clone(&self.temp_texture_1),
            ),
            _ => (
                Arc::clone(&self.bind_group_1),
                Arc::clone(&self.temp_texture_0),
            ),
        };
        self.temp_texture_index = (self.temp_texture_index + 1) % 2;
        ret
    }
    pub fn add_pipeline_from_shader(
        &mut self,
        label: Option<&str>,
        stage: RenderStage,
        fs_shader: ShaderModule,
        device: &Device,
        config: &SurfaceConfiguration,
    ) {
        let pipeline = device.create_render_pipeline(&wgpu_init::full_screen_pipeline_desc(
            label,
            &self.pipeline_layout,
            &self.vs_shader,
            &fs_shader,
            &[Some(wgpu_init::color_target_replace_write_all(
                config.format,
            ))],
        ));

        self.pipelines
            .entry(stage)
            .or_insert(Vec::new())
            .push(PostProcessingPipeline {
                pipeline: Arc::new(pipeline),
            });
    }

    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        device: &Device,
        config: &SurfaceConfiguration,
    ) {
        let bind_group_layout = &self.bind_group_layout;
        self.temp_texture_0 = Arc::new(create_color_render_target_image(
            width, height, device, config,
        ));
        self.temp_texture_1 = Arc::new(create_color_render_target_image(
            width, height, device, config,
        ));

        self.bind_group_0 = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&self.temp_texture_0.view);
            1: BindingResource::Sampler(&self.temp_texture_0.sampler);
        }));
        self.bind_group_1 = Arc::new(device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&self.temp_texture_1.view);
            1: BindingResource::Sampler(&self.temp_texture_1.sampler);
        }));
    }
}

impl FromWorld for PostProcessingManager {
    fn from_world(world: &mut World) -> Self {
        let rs = world.resource::<crate::RenderState>();

        let vs_shader = Arc::clone(&world.resource::<FullScreenVertexShader>().module);
        let descriptor = bg_layout_descriptor! {
            ["Post Processing"]
            0: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
            1: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
        };
        let bind_group_layout = rs.device.create_bind_group_layout(&descriptor);

        let size = world.resource::<RenderTargetSize>();
        let temp_texture_0 = Arc::new(create_color_render_target_image(
            size.width,
            size.height,
            &rs.device,
            &rs.config,
        ));
        let temp_texture_1 = Arc::new(create_color_render_target_image(
            size.width,
            size.height,
            &rs.device,
            &rs.config,
        ));

        let bind_group_0 = Arc::new(rs.device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&temp_texture_0.view);
            1: BindingResource::Sampler(&temp_texture_0.sampler);
        }));
        let bind_group_1 = Arc::new(rs.device.create_bind_group(&bg_descriptor! {
            ["Post Processing"] [&bind_group_layout]
            0: BindingResource::TextureView(&temp_texture_1.view);
            1: BindingResource::Sampler(&temp_texture_1.sampler);
        }));

        let pipeline_layout = rs.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Post Processing"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        Self {
            pipelines: HashMap::new(),
            bind_group_layout: Arc::new(bind_group_layout),
            bind_group_0,
            bind_group_1,
            pipeline_layout: Arc::new(pipeline_layout),
            temp_texture_0,
            temp_texture_1,
            vs_shader,
            temp_texture_index: 0,
        }
    }
}

#[derive(Clone)]
pub struct PostProcessingPipeline {
    pub pipeline: Arc<RenderPipeline>,
}
