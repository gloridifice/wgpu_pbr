use wgpu::{BindGroupLayoutEntry, BindingType, ShaderStages};

pub const fn bind_group_layout_entry_shader(binding: u32, ty: BindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::VERTEX,
        ty,
        count: None,
    }
}
