use std::borrow::Cow;

use bevy_ecs::prelude::Resource;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, ComputePassDescriptor, ComputePipelineDescriptor,
    PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};

use super::texture_preview::TexturePreview;

const DEPTH_TO_RGBA_SHADER: &str = r#"
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let depth_val = textureLoad(depth_tex, id.xy, 0);
    textureStore(out_tex, id.xy, vec4<f32>(depth_val, depth_val, depth_val, 1.0));
}
"#;

/// Holds the RGBA output for one converted depth layer, plus the cached
/// [`TexturePreview`] state so egui does not re-register every frame.
pub struct DepthLayerOutput {
    pub rgba_tex: wgpu::Texture,
    pub rgba_view: wgpu::TextureView,
    pub preview: TexturePreview,
    pub width: u32,
    pub height: u32,
    pub original_format: wgpu::TextureFormat,
}

/// Compute-pipeline resource that converts depth textures to RGBA8 for
/// display in egui.
///
/// Create once with [`DepthToRgbaConverter::new`], then call [`convert`]
/// each frame before the egui UI is built.
#[derive(Resource)]
pub struct DepthToRgbaConverter {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    outputs: Vec<DepthLayerOutput>,
}

impl DepthToRgbaConverter {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("depth_to_rgba shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(DEPTH_TO_RGBA_SHADER)),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("depth_to_rgba bgl"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Depth,
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("depth_to_rgba layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("depth_to_rgba pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            outputs: Vec::new(),
        }
    }

    /// Submit a command buffer that converts every `depth_view` to an RGBA
    /// output texture.  Existing outputs are reused when the dimensions and
    /// count match; otherwise they are recreated.
    pub fn convert(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        depth_views: &[&wgpu::TextureView],
        width: u32,
        height: u32,
        original_format: wgpu::TextureFormat,
    ) {
        self.ensure_outputs(device, depth_views.len(), width, height, original_format);

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        for (i, depth_view) in depth_views.iter().enumerate() {
            let output = &self.outputs[i];

            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("depth_to_rgba bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(depth_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&output.rgba_view),
                    },
                ],
            });

            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("depth_to_rgba"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups((width + 7) / 8, (height + 7) / 8, 1);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Mutable access to the per-layer outputs (including their
    /// [`TexturePreview`] caches) for use in egui UI code.
    pub fn outputs_mut(&mut self) -> &mut [DepthLayerOutput] {
        &mut self.outputs
    }

    // -----------------------------------------------------------------------
    // internals
    // -----------------------------------------------------------------------

    fn ensure_outputs(
        &mut self,
        device: &wgpu::Device,
        count: usize,
        width: u32,
        height: u32,
        original_format: wgpu::TextureFormat,
    ) {
        // Drop outputs whose size no longer matches.
        self.outputs.retain(|o| o.width == width && o.height == height);

        while self.outputs.len() < count {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth_to_rgba output"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&TextureViewDescriptor::default());

            self.outputs.push(DepthLayerOutput {
                rgba_tex: tex,
                rgba_view: view,
                preview: TexturePreview::new(),
                width,
                height,
                original_format,
            });
        }
    }
}
