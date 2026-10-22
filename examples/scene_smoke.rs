//! Explicit GPU/editor lifecycle check: cargo run --example scene_smoke.
//! Uses the normal window runner and real scene controller; creates only temporary assets.
use bevy_app::{App, AppExit, Last};
use bevy_ecs::prelude::*;
use lentille_render::{
    FrameSets,
    asset::{AssetRef, AssetRoot, MeshSource, SceneAssetCache, SubAsset},
    prelude::*,
    skybox::{Environment, Skybox},
};
use wgpu_pbr::{
    WgpuPbrPlugin,
    scene::{
        SceneController, SceneRequest,
        codec::{SceneCodec, SceneMetadata, SceneOwned, save_atomic},
        controller::{UnsavedDecision, snapshot},
    },
};

#[derive(Resource)]
struct Smoke {
    step: usize,
    frames: usize,
    original: Vec<Entity>,
    temp: tempfile::TempDir,
    saved: std::path::PathBuf,
    broken: std::path::PathBuf,
}
fn main() {
    color_backtrace::install();
    std::fs::create_dir_all("assets/.cache").unwrap();
    let temp = tempfile::tempdir_in("assets/.cache").unwrap();
    let relative = std::path::Path::new(".cache").join(temp.path().file_name().unwrap());
    let saved = relative.join("saved.scn.ron");
    let broken = relative.join("broken.scn.ron");
    let mut app = App::new();
    app.add_plugins(WgpuPbrPlugin)
        .insert_resource(Smoke {
            step: 0,
            frames: 0,
            original: Vec::new(),
            temp,
            saved,
            broken,
        })
        .add_systems(Last, check.after(FrameSets::Present));
    let result = app.run();
    assert!(matches!(result, AppExit::Success));
}
fn check(world: &mut World) {
    let mut smoke = world.remove_resource::<Smoke>().unwrap();
    smoke.frames += 1;
    assert!(
        smoke.frames < 180,
        "Scene smoke timed out at step {}",
        smoke.step
    );
    if world.resource::<SceneController>().path.is_none() {
        assert!(
            world.resource::<SceneController>().error.is_none(),
            "Startup failed: {:?}",
            world.resource::<SceneController>().error
        );
        world.insert_resource(smoke);
        return;
    }
    let path = world.resource::<SceneController>().path.clone().unwrap();
    let owned = |world: &mut World| {
        world
            .query_filtered::<Entity, With<SceneOwned>>()
            .iter(world)
            .collect::<Vec<_>>()
    };
    match smoke.step {
        0 => {
            assert_eq!(path, std::path::Path::new("scenes/minimal.scn.ron"));
            assert_eq!(world.resource::<SceneAssetCache>().counts(), (0, 2, 0));
            smoke.original = owned(world);
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open("scenes/demo.scn.ron".into()));
        }
        1 => {
            assert_eq!(path, std::path::Path::new("scenes/demo.scn.ron"));
            assert!(smoke.original.iter().all(|e| world.get_entity(*e).is_err()));
            assert_eq!(world.query::<&Camera>().iter(world).count(), 1);
            assert_eq!(world.resource::<SceneAssetCache>().counts().0, 3);
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open("scenes/minimal.scn.ron".into()));
        }
        2 => {
            assert_eq!(path, std::path::Path::new("scenes/minimal.scn.ron"));
            assert_eq!(world.resource::<SceneAssetCache>().counts(), (0, 2, 0));
            let root = world
                .query::<(Entity, &SceneMetadata)>()
                .iter(world)
                .next()
                .unwrap()
                .0;
            assert!(matches!(
                world.get::<Environment>(root),
                Some(Environment::Default)
            ));
            assert!(world.get::<Skybox>(root).unwrap().texture.is_none());
            let entity = world
                .query_filtered::<Entity, With<PbrMaterial>>()
                .iter(world)
                .next()
                .unwrap();
            world.get_mut::<PbrMaterial>(entity).unwrap().roughness = Some(0.37);
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open("scenes/demo.scn.ron".into()));
        }
        3 => {
            assert!(world.resource::<SceneController>().pending_open.is_some());
            world
                .resource_mut::<SceneController>()
                .resolve_unsaved(UnsavedDecision::Cancel);
            assert!(world.resource::<SceneController>().dirty);
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::SaveAs(smoke.saved.clone()));
        }
        4 => {
            assert_eq!(path, smoke.saved);
            assert!(!world.resource::<SceneController>().dirty);
            let codec = world.resource::<SceneCodec>().clone();
            let text = snapshot(world, &codec).unwrap();
            let mut scene = codec.decode(&text).unwrap();
            let mesh = scene
                .entities
                .iter_mut()
                .flat_map(|e| e.components.iter_mut())
                .find(|c| c.represents::<MeshSource>())
                .unwrap();
            *mesh = Box::new(MeshSource::Asset(AssetRef::new(
                ".cache/missing-smoke-model.glb",
                SubAsset::Mesh(0),
            )));
            save_atomic(
                world.resource::<AssetRoot>(),
                &smoke.broken,
                &codec.encode(&scene).unwrap(),
            )
            .unwrap();
            smoke.original = owned(world);
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open(smoke.broken.clone()));
        }
        5 => {
            assert_eq!(path, smoke.saved);
            assert!(
                world
                    .resource::<SceneController>()
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("missing-smoke-model")
            );
            assert!(smoke.original.iter().all(|e| world.get_entity(*e).is_ok()));
            assert_eq!(world.resource::<SceneAssetCache>().counts(), (0, 2, 0));
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Reload);
        }
        6 => {
            assert_eq!(path, smoke.saved);
            assert!(world.resource::<SceneController>().error.is_none());
            assert!(!world.resource::<SceneController>().dirty);
            assert!(
                world
                    .query::<&PbrMaterial>()
                    .iter(world)
                    .any(|m| m.roughness == Some(0.37))
            );
            assert!(smoke.temp.path().is_dir());
            let camera = world
                .query::<(Entity, &Camera)>()
                .iter(world)
                .next()
                .unwrap()
                .0;
            world.get_mut::<Camera>(camera).unwrap().fovy = 60.0;
            world
                .get_mut::<lentille_render::shadow::csm::CsmConfig>(camera)
                .unwrap()
                .texture_size = 256;
        }
        7 => { /* Allow the new CSM allocation to finish before checking matrices. */ }
        8 => {
            let csm = world
                .query::<&lentille_render::shadow::csm::CascadeShadowMapping>()
                .iter(world)
                .next()
                .unwrap();
            assert!(
                csm.initialized,
                "rebuilt CSM must initialize without camera movement"
            );
            assert_eq!(csm.config.texture_size, 256);
            println!(
                "SCENE_SMOKE_OK: GPU scenes switched, edits protected, disk save/reload, failed candidate rollback, scope reset, CSM reconfiguration"
            );
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    smoke.step += 1;
    world.insert_resource(smoke);
}
