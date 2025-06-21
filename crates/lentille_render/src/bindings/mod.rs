use bevy_app::Plugin;

use crate::{ColorRenderTarget, app_ext::AppExt, prelude::after};

pub mod global_binding;
pub mod light_binding;
pub mod material_binding;
pub mod object_binding;

pub(crate) struct BindingsPlugin;

impl Plugin for BindingsPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_render_resource::<object_binding::ObjectBindGroupLayout>()
            .init_render_resource::<material_binding::PBRMaterialBindGroupLayout>()
            .init_render_resource::<global_binding::GlobalBindGroup>()
            .init_render_resource_with_config::<global_binding::GlobalUniformBuffer>([after::<
                ColorRenderTarget,
            >(
            )]);
    }
}
