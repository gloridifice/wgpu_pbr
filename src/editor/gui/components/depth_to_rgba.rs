use std::borrow::Cow;

use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, ComputePassDescriptor, ComputePipelineDescriptor,
    PipelineLayoutDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, TextureFormat, TextureSampleType, TextureViewDimension,
};

// ---------------------------------------------------------------------------
// Shared WGSL
// ---------------------------------------------------------------------------

const DEPTH_TO_RGBA_SHADER: &str = include_str!("../../shaders/depth_to_rgba.wgsl");

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
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
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
        cpass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
}
