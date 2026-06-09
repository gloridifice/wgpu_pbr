use std::borrow::Cow;

use bevy_ecs::prelude::Component;
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, ComputePassDescriptor, ComputePipelineDescriptor,
    PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};

use super::texture_preview::TexturePreview;

// ---------------------------------------------------------------------------
// Shared WGSL
// ---------------------------------------------------------------------------

const DEPTH_TO_RGBA_SHADER: &str = r#"
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(0) @binding(1) var out_tex: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let depth_val = textureLoad(depth_tex, id.xy, 0);
    textureStore(out_tex, id.xy, vec4<f32>(depth_val, depth_val, depth_val, 1.0));
}
"#;

// ---------------------------------------------------------------------------
// General-purpose depth → RGBA compute pipeline
// ---------------------------------------------------------------------------

/// Stateless compute pipeline that converts a single depth texture view
/// into a [`TextureFormat::Rgba8Unorm`] output.
///
/// Call [`convert_to`](Self::convert_to) to record one dispatch into an
/// existing [`wgpu::CommandEncoder`].
pub struct DepthToRgbaConverter {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
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
        }
    }

    /// Record one dispatch that reads `depth_view` and writes into
    /// `output_view`.  Caller is responsible for creating both the encoder
    /// and the output texture (with [`TextureUsages::STORAGE_BINDING`]).
    pub fn convert_to(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
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
                    resource: BindingResource::TextureView(output_view),
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
}

// ---------------------------------------------------------------------------
// CSM-specific wrapper (output management + preview state)
// ---------------------------------------------------------------------------

/// Holds the RGBA output for one converted CSM depth layer, plus the
/// cached [`TexturePreview`] state so egui does not re-register every frame.
pub struct CsmDepthLayerOutput {
    pub rgba_tex: wgpu::Texture,
    pub rgba_view: wgpu::TextureView,
    pub preview: TexturePreview,
    pub width: u32,
    pub height: u32,
    pub original_format: wgpu::TextureFormat,
}

/// Manages per-frame depth→RGBA conversion of CSM layers and caches
/// the output textures + [`TexturePreview`] state for the egui UI.
#[derive(Component)]
pub struct CsmDepthToRgbaConverter {
    inner: DepthToRgbaConverter,
    outputs: Vec<CsmDepthLayerOutput>,
}

impl CsmDepthToRgbaConverter {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            inner: DepthToRgbaConverter::new(device),
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
            self.inner.convert_to(
                device,
                &mut encoder,
                depth_view,
                &output.rgba_view,
                width,
                height,
            );
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Mutable access to the per-layer outputs (including their
    /// [`TexturePreview`] caches) for use in egui UI code.
    pub fn outputs_mut(&mut self) -> &mut [CsmDepthLayerOutput] {
        &mut self.outputs
    }

    // -------------------------------------------------------------------
    fn ensure_outputs(
        &mut self,
        device: &wgpu::Device,
        count: usize,
        width: u32,
        height: u32,
        original_format: wgpu::TextureFormat,
    ) {
        self.outputs.retain(|o| o.width == width && o.height == height);

        while self.outputs.len() < count {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("csm_depth_to_rgba output"),
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

            self.outputs.push(CsmDepthLayerOutput {
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
