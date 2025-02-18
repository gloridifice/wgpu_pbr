use std::sync::Arc;

use bevy_ecs::{system::Resource, world::FromWorld};
use wgpu::{ShaderModule, TextureFormat};

use crate::asset::load::Loadable;

#[derive(Clone, Resource)]
pub struct DefaultMipmapGenShader {
    shader: Arc<ShaderModule>,
}

impl FromWorld for DefaultMipmapGenShader {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        Self {
            shader: Arc::new(
                ShaderModule::load(
                    crate::asset::AssetPath::Assets("shaders/blit.wgsl".to_string()),
                    world,
                )
                .unwrap(),
            ),
        }
    }
}

pub fn calculate_mip_level_count<const N: usize>(tex_size: &[u32; N]) -> u32 {
    tex_size
        .iter()
        .max()
        .map(|max| {
            return 1u32 + (*max as f32).log2() as u32;
        })
        .unwrap_or(1u32)
}

pub fn generate_mip_map(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    texture: &wgpu::Texture,
    format: TextureFormat,
    shader: &wgpu::ShaderModule,
    mip_count: u32,
) {
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("blit"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let bind_group_layout = pipeline.get_bind_group_layout(0);

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mip"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let views = (0..mip_count)
        .map(|level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mip"),
                format: None,
                dimension: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: level,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
                usage: None,
            })
        })
        .collect::<Vec<_>>();

    for target_mip in 1..mip_count as usize {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[target_mip - 1]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: None,
        });

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &views[target_mip],
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..4, 0..1);
    }
}
