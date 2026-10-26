use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use lentille_render::{
    RenderState,
    asset::sys_prepare_assets,
    skybox::{Environment, Skybox, SkyboxSHBuffer, prepare_environment},
};
pub mod browser;
pub mod codec;
pub mod controller;
pub mod loading;
mod snapshot_cache;
pub use controller::{SceneController, SceneRequest};
pub use loading::SceneLoadSettings;

pub(crate) fn scene_editable(controller: Res<SceneController>) -> bool {
    !controller.read_only()
}
pub struct ScenePlugin;
#[derive(Component)]
pub(crate) struct EnvironmentState {
    pub source: Environment,
    pub error: Option<String>,
}
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        let codec = codec::SceneCodec::new();
        app.insert_resource(codec.registry.clone())
            .insert_resource(codec)
            .insert_resource(SceneController {
                request: Some(SceneRequest::Open("scenes/minimal.scn.ron".into())),
                ..Default::default()
            })
            .init_resource::<SceneLoadSettings>()
            .init_resource::<loading::SceneLoader>()
            .configure_sets(
                PostUpdate,
                lentille_render::asset::AssetPreparationSet.run_if(scene_editable),
            )
            .add_systems(
                PostUpdate,
                (controller::sys_scene_requests, sys_environment)
                    .chain()
                    .before(sys_prepare_assets),
            );
    }
}
fn sys_environment(world: &mut World) {
    if !world.contains_resource::<lentille_render::RenderReady>()
        || world.resource::<SceneController>().read_only()
    {
        return;
    }
    let removed: Vec<_> = world
        .query_filtered::<Entity, (
            Or<(With<Skybox>, With<EnvironmentState>)>,
            Without<Environment>,
        )>()
        .iter(world)
        .collect();
    for entity in removed {
        world
            .entity_mut(entity)
            .remove::<(Skybox, EnvironmentState)>();
        let sh = world
            .resource::<lentille_render::skybox::DefaultSkybox>()
            .sh;
        world
            .resource::<SkyboxSHBuffer>()
            .buffer
            .write(sh, &world.resource::<RenderState>().queue);
    }
    let requests: Vec<_> = world
        .query::<(
            Entity,
            &Environment,
            Option<&EnvironmentState>,
            Option<&Skybox>,
        )>()
        .iter(world)
        .filter(|(_, source, state, skybox)| {
            state.is_none_or(|v| v.source != **source || (v.error.is_none() && skybox.is_none()))
        })
        .map(|(e, s, _, _)| (e, s.clone()))
        .collect();
    for (e, source) in requests {
        let error = match prepare_environment(world, &source) {
            Ok((skybox, sh)) => {
                world.entity_mut(e).insert(skybox);
                world
                    .resource::<SkyboxSHBuffer>()
                    .buffer
                    .write(sh, &world.resource::<RenderState>().queue);
                None
            }
            Err(error) => {
                world.entity_mut(e).remove::<Skybox>();
                let sh = world
                    .resource::<lentille_render::skybox::DefaultSkybox>()
                    .sh;
                world
                    .resource::<SkyboxSHBuffer>()
                    .buffer
                    .write(sh, &world.resource::<RenderState>().queue);
                Some(format!("{error:#}"))
            }
        };
        if let Some(error) = &error {
            world.resource_mut::<SceneController>().error = Some(error.clone());
        }
        world
            .entity_mut(e)
            .insert(EnvironmentState { source, error });
    }
}
