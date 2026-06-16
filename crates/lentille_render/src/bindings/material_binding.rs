use crate::{material::pbr::PbrMaterialUniform, prelude::*};
use bevy_ecs::prelude::*;
use lentille_wgpu_macros::DeviceNewFromWorld;
use lentille_wgpu_utils::typed_sampler::FilteringSampler;

binding_define! {
    [PbrMaterial]
    layout_macro: #[derive(Resource, DeviceNewFromWorld)]
    0: frag => pbr_uniform: TypedBuffer<PbrMaterialUniform>,
    1: frag => sampler: FilteringSampler,
    2: frag => base_color_texture: TexView2D<SampleFloatFilterable>,
    3: frag => normal_texture: TexView2D<SampleFloatFilterable>,
    4: frag => metallic_roughness_ao_texture: TexView2D<SampleFloatFilterable>,
    5: frag => emission_texture: TexView2D<SampleFloatFilterable>,
}
