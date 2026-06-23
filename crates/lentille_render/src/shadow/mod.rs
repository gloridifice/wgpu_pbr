use bevy_app::Plugin;
use bevy_ecs::prelude::*;
use csm::CsmPlugin;

pub mod csm;

pub(crate) struct ShadowPlugin;

impl Plugin for ShadowPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(CsmPlugin);
    }
}

#[derive(Component, Clone, Default)]
pub struct CastShadow;
