use crate::{material::pbr::RawPBRMaterial, prelude::*};
use bevy_ecs::prelude::*;
use lentille_wgpu_macros::DeviceNewFromWorld;
use lentille_wgpu_utils::typed_sampler::FilteringSampler;

binding_define! {
    [PbrMaterial]
    layout_macro: #[derive(Resource, DeviceNewFromWorld)]
    0: frag => pbr_uniform: TypedBuffer<RawPBRMaterial>,
    1: frag => base_color_texture: TexView2D<SampleFloatFilterable>,
    2: frag => base_color_sampler: FilteringSampler,
    3: frag => normal_texture: TexView2D<SampleFloatFilterable>,
    4: frag => normal_sampler: FilteringSampler,
}
