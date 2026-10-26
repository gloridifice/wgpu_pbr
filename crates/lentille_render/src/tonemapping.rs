//! Scene-linear HDR to SDR. Display overlays are deliberately scheduled afterwards.
use bevy_app::Plugin;
use bevy_ecs::prelude::*;

use crate::{SCREEN_FORMAT, ToneMappingStage, prelude::*, stage::RenderContext};

pub(crate) struct ToneMappingPlugin;

impl Plugin for ToneMappingPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<ToneMappingSettings>()
            .init_render_resource_with_config::<ToneMappingPipeline>([after::<
                FullScreenVertexShader,
            >()])
            .add_render_system(sys_tone_mapping, ToneMappingStage);
    }
}

/// Runtime-only exposure compensation in stops; positive values brighten the scene.
#[derive(Resource, Default, Clone, Copy)]
pub struct ToneMappingSettings {
    pub exposure_compensation_ev: f32,
}

impl ToneMappingSettings {
    fn multiplier(self) -> f32 {
        let ev = self.exposure_compensation_ev;
        if ev.is_finite() {
            ev.clamp(-16.0, 16.0).exp2()
        } else {
            1.0
        }
    }
}

#[derive(Resource)]
pub struct ToneMappingPipeline {
    pipeline: RenderPipeline,
    layout: BindGroupLayout,
    exposure: wgpu::Buffer,
}

impl FromWorld for ToneMappingPipeline {
    fn from_world(world: &mut World) -> Self {
        let source = world
            .resource_mut::<ShaderLoader>()
            .load_source(AssetPath::new_shader_wgsl("postprocessing/tonemap"))
            .expect("tone mapping shader");
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tone Mapping"),
            source,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tone Mapping"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tone Mapping"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline =
            device.create_render_pipeline(&lentille_wgpu_utils::full_screen_pipeline_desc(
                Some("Tone Mapping"),
                &pipeline_layout,
                &world.resource::<FullScreenVertexShader>().module,
                &shader,
                &[Some(lentille_wgpu_utils::color_target_replace_write_all(
                    SCREEN_FORMAT,
                ))],
            ));
        let exposure = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tone Mapping Exposure"),
            contents: bytemuck::cast_slice(&[1.0f32, 0.0, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        Self {
            pipeline,
            layout,
            exposure,
        }
    }
}

impl ToneMappingPipeline {
    pub fn render(
        &self,
        rs: &RenderState,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        settings: ToneMappingSettings,
    ) {
        rs.queue.write_buffer(
            &self.exposure,
            0,
            bytemuck::cast_slice(&[settings.multiplier(), 0.0, 0.0, 0.0]),
        );
        let bindings = rs.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tone Mapping"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.exposure.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tone Mapping"),
            color_attachments: &[Some(lentille_wgpu_utils::render_pass_color_attachment(
                target,
                Some(wgpu::Color::BLACK),
                true,
            ))],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub fn sys_tone_mapping(
    ctx: InMut<RenderContext>,
    rs: Res<RenderState>,
    pipeline: Res<ToneMappingPipeline>,
    settings: Res<ToneMappingSettings>,
) {
    let InMut(RenderContext {
        encoder,
        hdr,
        display_target,
        ..
    }) = ctx;
    pipeline.render(
        &rs,
        encoder,
        &hdr.current().view,
        &display_target.view,
        *settings,
    );
}

#[cfg(test)]
mod tests;
