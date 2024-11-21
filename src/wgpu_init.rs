use wgpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

pub const fn bind_group_layout_entry_shader(binding: u32, ty: BindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::VERTEX,
        ty,
        count: None,
    }
}

pub const fn uniform_buffer_bg_layout_entry(
    binding: u32,
    visibility: ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
