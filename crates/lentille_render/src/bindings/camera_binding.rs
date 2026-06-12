use crate::{
    camera::CameraUniform, light::LightUniform, prelude::*, shadow_mapping::csm::GpuCsmInfoUniform,
    skybox::ComputedSHUniform,
};
use bevy_ecs::prelude::*;
use lentille_wgpu_utils::{typed_sampler::*, typed_texture::*};

binding_define! {
    [Camera]
    layout_macro: #[derive(Resource)],
    0: all   => camera_uniform: TypedBuffer<CameraUniform>,
    1: all   => light_uniform: TypedBuffer<LightUniform>,
    2: frag  => csm_buffer: TexView2DArray<SampleDepth>,
    3: frag  => csm_sampler: ComparisonSampler,
    4: frag  => dfg: TexView2D<SampleFloatFilterable>,
    5: frag  => skybox: TexViewCube<SampleFloatFilterable>,
    6: frag  => skybox_sampler: FilteringSampler,
    7: frag  => skybox_sh: TypedBuffer<ComputedSHUniform>,
    8: frag  => color_target: TexView2D<SampleFloatFilterable>,
    9: frag  => color_target_sampler: NonFilteringSampler,
    10: frag => csm_info: TypedBuffer<GpuCsmInfoUniform>,
}

impl FromWorld for CameraBindGroupLayout {
    fn from_world(world: &mut World) -> Self {
        Self::new(&world.resource::<RenderState>().device)
    }
}
