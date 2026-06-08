use bevy_app::Plugin;
use bevy_ecs::{resource::Resource, world::World};
use wgpu::{
    CommandEncoderDescriptor, FragmentState, PipelineLayoutDescriptor, RenderPassColorAttachment,
    RenderPipelineDescriptor, TextureView, VertexState,
};

use crate::prelude::*;

pub(crate) struct BlitPlugin;

impl Plugin for BlitPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<BlitShader>();
    }
}

#[derive(Resource)]
pub struct BlitShader(pub Arc<ShaderModule>);

#[derive(Resource)]
pub struct BlitPipeline {
    pipeline: Arc<RenderPipeline>,
    bind_group_layout: Arc<BindGroupLayout>,
    #[allow(unused)]
    pipeline_layout: Arc<PipelineLayout>,
}

impl FromWorld for BlitShader {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<RenderState>();
        let shader = rs
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Blit Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
                @vertex
                fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {
                    var pos = array<vec2<f32>, 3>(
                        vec2<f32>(-1.0, -3.0),
                        vec2<f32>( 3.0,  1.0),
                        vec2<f32>(-1.0,  1.0)
                    );
                    return vec4<f32>(pos[idx], 0.0, 1.0);
                }

                @group(0) @binding(0) var myTex: texture_2d<f32>;
                @group(0) @binding(1) var mySampler: sampler;

                @fragment
                fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
                    let dims = vec2<f32>(textureDimensions(myTex, 0));
                    let uv = frag_coord.xy / dims;
                    return textureSample(myTex, mySampler, uv);
                }
            "#
                    .into(),
                ),
            });

        Self(Arc::new(shader))
    }
}

impl BlitPipeline {
    pub fn new(world: &mut World, surface_format: wgpu::TextureFormat) -> Self {
        let shader = world.resource::<BlitShader>();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let bgl = device.create_bind_group_layout(&bg_layout_descriptor! {
            ["Blit"]
            0: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, TextureSampleType::Float { filterable: true });
            1: ShaderStages::FRAGMENT => BGLEntry::Sampler(SamplerBindingType::Filtering);
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Blit"),
            bind_group_layouts: &[Some(&bgl)],
            ..Default::default()
        });

        let pipeline = Arc::new(rs.device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader.0,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module: &shader.0,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(surface_format.into())],
            }),
            primitive: Default::default(),
            multisample: Default::default(),
            depth_stencil: None,
            multiview_mask: None,
            cache: None,
        }));

        Self {
            pipeline,
            bind_group_layout: Arc::new(bgl),
            pipeline_layout: Arc::new(pipeline_layout),
        }
    }
}

pub fn blit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    blit: &BlitPipeline,
    sampler: &Sampler,
    source: &TextureView,
    dst: &TextureView,
) {
    let bg = device.create_bind_group(&bg_descriptor! {
       ["Blit"] [&blit.bind_group_layout]
       0: BindingResource::TextureView(source);
       1: BindingResource::Sampler(sampler);
    });

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Blit"),
    });

    {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Blit render pass"),
            multiview_mask: None,
            color_attachments: &[Some(RenderPassColorAttachment {
                view: dst,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&blit.pipeline);
        render_pass.set_bind_group(0, &bg, &[]);
        render_pass.draw(0..3, 0..1);
    }

    queue.submit(std::iter::once(encoder.finish()));
}
