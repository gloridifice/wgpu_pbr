use wgpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

#[derive(Clone)]
pub enum BGLEntry {
    UniformBuffer(),
    Tex2D(bool, wgpu::TextureSampleType),
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

            BGLEntry::UniformBuffer() => {
                let ty = BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                };

                BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty,
                    count: None,
                }
            }
            BGLEntry::Tex2D(multisampled, texture_sample_type) => {
                let ty = BindingType::Texture {
                    sample_type: texture_sample_type,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled,
                };

                BindGroupLayoutEntry {
                    binding,
                    visibility,
                    ty,
                    count: None,
                }
            }

            BGLEntry::Sampler(sampler_binding_type) => {
                let ty = wgpu::BindingType::Sampler(sampler_binding_type);

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