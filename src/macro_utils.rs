use wgpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

#[allow(unused)]
#[derive(Clone)]
pub enum BGLEntry {
    UniformBuffer(),
    /// `(is_read_only: bool)`
    StorageBuffer(bool),
    /// `(multisampled: bool, texture_sample_type: wgpu::TextureSampleType)`
    Tex2D(bool, wgpu::TextureSampleType),
    TexCube(bool, wgpu::TextureSampleType),
    Sampler(wgpu::SamplerBindingType),
    Raw(BindGroupLayoutEntry),
}

impl BGLEntry {
    pub const fn into_bgl_entry(
        self,
        binding: u32,
        visibility: ShaderStages,
    ) -> BindGroupLayoutEntry {
        match self {
            BGLEntry::Raw(mut bind_group_layout_entry) => {
                bind_group_layout_entry.binding = binding;
                bind_group_layout_entry.visibility = visibility;
                bind_group_layout_entry
            }
            _ => {
                let ty = match self {
                    BGLEntry::UniformBuffer() => BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    BGLEntry::StorageBuffer(read_only) => wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    BGLEntry::Tex2D(multisampled, texture_sample_type) => BindingType::Texture {
                        sample_type: texture_sample_type,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled,
                    },
                    BGLEntry::TexCube(multisampled, texture_sample_type) => BindingType::Texture {
                        sample_type: texture_sample_type,
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled,
                    },
                    BGLEntry::Sampler(sampler_binding_type) => {
                        wgpu::BindingType::Sampler(sampler_binding_type)
                    }
                    BGLEntry::Raw(_) => BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                };

                BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty,
                    count: None,
                }
            }
        }
    }
}

/// ```
/// bgl_entries!{
///     1: frag => IntoBGLEntry::UniformBuffer()
/// }
/// ```
#[macro_export]
macro_rules! bgl_entries {
    ($($i:literal: $vis:expr => $c:expr;)*) => {
        [
            $($c.into_bgl_entry($i, $vis),)*
        ]
    };

    () => ()
}
/// ```
/// let descriptor = bg_layout_descriptor!{
///     ["Post Processing"]
///     0: ShaderStages::FRAGMENT => BGLEntry::Tex2D(false, wgpu::TextureSampleType::Float { filterable: true });
///     1: ShaderStages::FRAGMENT => BGLEntry::Sampler(wgpu::SamplerBindingType::Filtering);
/// };
/// ```
#[macro_export]
macro_rules! bg_layout_descriptor {
    ([$name:literal] $($i:literal: $vis:expr => $c:expr;)*) => {
        wgpu::BindGroupLayoutDescriptor {
            label: Some($name),
            entries: &[
                $($c.into_bgl_entry($i, $vis),)*
            ]
        }
    };

    () => ()
}

/// ## Usage
/// ```
/// let bind_group_layout = ...;
/// let bind_group_desc = bg_descriptor!(
///     ["PBR Material Bind Group"] [bind_group_layout]
///     0: BindingResource::TextureView(&base_color.view);
///     1: BindingResource::Sampler(&base_color.sampler);
/// );
/// ```
#[macro_export]
macro_rules! bg_descriptor {
    ([$name:literal] [$layout:expr] $($i:literal: $c:expr;)*) => {
        wgpu::BindGroupDescriptor {
            label: Some($name),
            layout: $layout,
            entries: &[
                $(wgpu::BindGroupEntry{
                    binding: $i,
                    resource: $c,
                },)*
            ]
        }
    };
}

#[macro_export]
macro_rules! impl_pod_zeroable {
    ($A: ty) => {
        unsafe impl bytemuck::Pod for $A {}
        unsafe impl bytemuck::Zeroable for $A {}
    };
}

#[macro_export]
macro_rules! static_render_target_id {
    ($a:expr) => {
        pub static $a: LazyLock<RTId> = LazyLock::new(|| RTId::new());
    };
}
