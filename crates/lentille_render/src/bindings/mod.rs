use bevy_app::Plugin;

use crate::{
    app_ext::AppExt,
    bindings::{
        camera_binding::CameraBindGroupLayout, light_binding::DynamicLightBindGroupLayout,
        material_binding::PbrMaterialBindGroupLayout, object_binding::ObjectBindGroupLayout,
    },
    graph::after,
    prelude::DynamicLightBindGroup,
};

pub mod camera_binding;
pub mod light_binding;
pub mod material_binding;
pub mod object_binding;

pub(crate) struct BindingsPlugin;

impl Plugin for BindingsPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<ObjectBindGroupLayout>()
            .init_render_resource::<PbrMaterialBindGroupLayout>()
            .init_render_resource::<DynamicLightBindGroupLayout>()
            .init_render_resource::<CameraBindGroupLayout>()
            .init_render_resource_with_config::<DynamicLightBindGroup>([after::<
                DynamicLightBindGroupLayout,
            >()]);
    }
}
