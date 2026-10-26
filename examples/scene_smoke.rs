//! Explicit GPU/editor lifecycle check: cargo run --example scene_smoke.
//! Uses the normal window runner and real scene controller; creates only temporary assets.
use bevy_app::{App, AppExit, Last};
use bevy_ecs::prelude::*;
use lentille_render::{
    FrameSets,
    asset::{AssetRef, AssetRoot, MeshSource, SceneAssetCache, SubAsset},
    picking::resources::{LastPickResult, PickEvent, PickReadback, PickRequest, SelectedObjects},
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
    started: std::time::Instant,
    loading_frames: usize,
    protected_snapshot: Option<String>,
    resized_while_loading: bool,
    picked: Option<Entity>,
    camera: Option<Entity>,
    unselected_pixels: Vec<u8>,
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
            started: std::time::Instant::now(),
            loading_frames: 0,
            protected_snapshot: None,
            resized_while_loading: false,
            picked: None,
            camera: None,
            unselected_pixels: Vec::new(),
        })
        .add_systems(Last, check.after(FrameSets::Present));
    let result = app.run();
    assert!(matches!(result, AppExit::Success));
}
fn display_pixels(world: &World, camera: Entity) -> Vec<u8> {
    let rs = world.resource::<RenderState>();
    let image = world
        .get::<lentille_render::camera::RenderTarget>(camera)
        .unwrap()
        .display_color();
    lentille_wgpu_utils::texture_readback::read_texture_to_cpu(
        &rs.device,
        &rs.queue,
        image.texture.texture(),
    )
    .data
}

fn check(world: &mut World) {
    let mut smoke = world.remove_resource::<Smoke>().unwrap();
    smoke.frames += 1;
    assert!(
        smoke.frames < 50_000 && smoke.started.elapsed() < std::time::Duration::from_secs(120),
        "Scene smoke timed out at step {}",
        smoke.step
    );
    if let Some(progress) = world.resource::<SceneController>().loading.clone() {
        smoke.loading_frames += 1;
        if let Some(expected) = &smoke.protected_snapshot {
            let codec = world.resource::<SceneCodec>().clone();
            assert_eq!(
                &snapshot(world, &codec).unwrap(),
                expected,
                "Loading must freeze persistent scene data"
            );
            assert!(smoke.original.iter().all(|e| world.get_entity(*e).is_ok()));
        }
        if !smoke.resized_while_loading && world.resource::<SceneController>().path.is_some() {
            let mut query = world.query::<&mut lentille_render::camera::RenderTargetSize>();
            if let Some(mut size) = query.iter_mut(world).next() {
                size.width = 640;
                size.height = 360;
            }
            smoke.resized_while_loading = true;
        }
        if smoke.step == 9 && progress.stage == "Preparing GPU" {
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::CancelLoad);
            smoke.step = 10;
        }
        world.insert_resource(smoke);
        return;
    }
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
            let animated = world
                .query_filtered::<Entity, With<MeshSource>>()
                .iter(world)
                .next()
                .unwrap();
            world
                .entity_mut(animated)
                .insert(wgpu_pbr::RotationObject { speed: 1.0 });
            world.resource_mut::<SceneController>().discard_next_open = true;
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open("scenes/demo.scn.ron".into()));
        }
        1 => {
            assert_eq!(path, std::path::Path::new("scenes/demo.scn.ron"));
            assert!(
                smoke.loading_frames > 1,
                "Demo GPU preparation must span frames"
            );
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
            world.resource_mut::<SceneController>().discard_next_open = true;
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open("scenes/demo.scn.ron".into()));
        }
        10 => {
            assert_eq!(path, smoke.saved);
            assert!(
                world.resource::<SceneController>().dirty,
                "GPU cancellation must preserve unsaved CSM edits"
            );
            assert!(world.resource::<SceneController>().error.is_none());
            assert!(smoke.original.iter().all(|e| world.get_entity(*e).is_ok()));
            assert_eq!(world.resource::<SceneAssetCache>().counts(), (0, 2, 0));
            world.resource_mut::<SceneController>().discard_next_open = true;
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open(smoke.saved.clone()));
        }
        11 => {
            assert_eq!(path, smoke.saved);
            assert!(!world.resource::<SceneController>().dirty);
            assert!(smoke.resized_while_loading);
            let codec = world.resource::<SceneCodec>().clone();
            let mut candidate = codec.decode(&snapshot(world, &codec).unwrap()).unwrap();
            let target = candidate
                .entities
                .iter_mut()
                .flat_map(|e| e.components.iter_mut())
                .find(|c| c.represents::<lentille_render::camera::RenderTargetConfig>())
                .unwrap();
            *target = Box::new(lentille_render::camera::RenderTargetConfig::Texture {
                width: world
                    .resource::<lentille_render::RenderState>()
                    .device
                    .limits()
                    .max_texture_dimension_2d
                    + 1,
                height: 1,
            });
            save_atomic(
                world.resource::<AssetRoot>(),
                &smoke.broken,
                &codec.encode(&candidate).unwrap(),
            )
            .unwrap();
            world
                .resource_mut::<SceneController>()
                .request(SceneRequest::Open(smoke.broken.clone()));
        }
        12 => {
            assert_eq!(path, smoke.saved);
            assert!(smoke.original.iter().all(|e| world.get_entity(*e).is_ok()));
            assert!(
                world
                    .resource::<SceneController>()
                    .error
                    .as_ref()
                    .unwrap()
                    .contains("device limits")
            );
            // Freeze fixture animation before comparing selected/unselected output.
            for mut rotation in world
                .query::<&mut wgpu_pbr::RotationObject>()
                .iter_mut(world)
            {
                rotation.speed = 0.0;
            }
        }
        13 => {
            let camera = world
                .query::<&SceneMetadata>()
                .single(world)
                .unwrap()
                .active_camera;
            let (entity, position) = world
                .query::<(Entity, &Name, &WorldTransform)>()
                .iter(world)
                .find(|(_, name, transform)| {
                    name.as_str() == "Material sample" && transform.position.x.abs() < 0.1
                })
                .map(|(e, _, t)| (e, t.position))
                .unwrap();
            let camera_state = world
                .get::<lentille_render::camera::CameraState>(camera)
                .unwrap();
            let clip = camera_state.view_proj * position.extend(1.0);
            assert!(clip.w > 0.0);
            let ndc = clip.truncate() / clip.w;
            assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0);
            let size = world
                .get::<lentille_render::camera::RenderTargetSize>(camera)
                .unwrap();
            let target_size = lentille_math::Vec2::new(size.width as f32, size.height as f32);
            let screen_pos = lentille_math::Vec2::new(
                (ndc.x + 1.0) * 0.5 * target_size.x,
                (1.0 - ndc.y) * 0.5 * target_size.y,
            );
            smoke.unselected_pixels = display_pixels(world, camera);
            smoke.picked = Some(entity);
            smoke.camera = Some(camera);
            world.insert_resource(PickRequest {
                camera_entity: camera,
                target_size,
                screen_pos,
            });
        }
        14 => {
            if world.contains_resource::<PickRequest>()
                || !world.resource::<PickReadback>().is_idle()
            {
                world.insert_resource(smoke);
                return;
            }
            assert_eq!(
                world.resource::<LastPickResult>().entity,
                smoke.picked,
                "batched pick must return the requested visible entity"
            );
            assert!(
                world
                    .resource::<SelectedObjects>()
                    .is_selected(smoke.picked.unwrap())
            );
        }
        15 => { /* Allow outline group synchronization after the pick observer. */ }
        16 => {
            assert_ne!(
                display_pixels(world, smoke.camera.unwrap()),
                smoke.unselected_pixels,
                "selection must render a visible outline"
            );
            world.trigger(PickEvent::new(None, lentille_math::Vec2::new(0.0, 0.0)));
        }
        17 => { /* Allow deselection to retire outline layers. */ }
        18 => {
            assert!(world.resource::<SelectedObjects>().entities.is_empty());
            assert_eq!(
                display_pixels(world, smoke.camera.unwrap()),
                smoke.unselected_pixels,
                "deselection must restore the unselected display"
            );
            println!(
                "SCENE_SMOKE_OK: async frames {}, frozen edits, resize, cancellation, rollback, save/reload, CSM, batched picking and outline select/deselect",
                smoke.loading_frames
            );
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    if matches!(
        world.resource::<SceneController>().request,
        Some(SceneRequest::Open(_) | SceneRequest::Reload)
    ) {
        smoke.original = owned(world);
        let codec = world.resource::<SceneCodec>().clone();
        smoke.protected_snapshot = Some(snapshot(world, &codec).unwrap());
    } else {
        smoke.protected_snapshot = None;
    }
    smoke.step += 1;
    world.insert_resource(smoke);
}
