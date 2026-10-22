use bevy_ecs::prelude::*;
use lentille_render::{
    asset::{AssetRoot, MeshSource},
    camera::RenderTargetConfig,
    prelude::*,
    shadow::csm::CsmConfig,
    skybox::Environment,
};
use std::{fs, path::Path};
use wgpu_pbr::scene::{
    browser::{BrowserEntryKind, scan_scene_directory},
    codec::{SceneCodec, SceneMetadata, SceneOwned, save_atomic, validate},
};

fn fixture(codec: &SceneCodec) -> World {
    let mut world = World::new();
    world.insert_resource(codec.registry.clone());
    let camera = world
        .spawn((
            Name::new("Camera"),
            Camera::new(1.),
            RenderTargetConfig::Viewport,
            Transform::default(),
            CsmConfig {
                level_count: 2,
                texture_size: 256,
                linear_log_factor: 0.5,
                shadow_near: 1.,
                shadow_far: 50.,
            },
        ))
        .id();
    world.spawn((
        SceneMetadata {
            name: "Fixture".into(),
            active_camera: camera,
        },
        Environment::Default,
    ));
    world.spawn((
        Name::new("Sun"),
        ParallelLight::default(),
        Transform::default(),
    ));
    let parent = world
        .spawn((
            Name::new("Parent"),
            Transform::new().position(Vec3::new(3., 0., 0.)),
        ))
        .id();
    world.spawn((
        Name::new("Child"),
        Transform {
            parent: Some(parent),
            position: Vec3::new(0., 2., 0.),
            ..Default::default()
        },
        MeshSource::Plane { size: 2. },
        PbrMaterial {
            roughness: Some(0.25),
            color: Some(Color::RED),
            ..Default::default()
        },
        SceneOwned,
    ));
    world
}
fn encode(codec: &SceneCodec, world: &mut World) -> String {
    let ids: Vec<_> = world.query::<Entity>().iter(world).collect();
    codec
        .encode(&codec.capture(world, ids.into_iter()).unwrap())
        .unwrap()
}

#[test]
fn disk_round_trip_restores_components_and_remaps_parent_in_a_nonempty_world() {
    let codec = SceneCodec::new();
    let mut world = fixture(&codec);
    let text = encode(&codec, &mut world);
    let dir = tempfile::tempdir().unwrap();
    let root = AssetRoot(dir.path().into());
    save_atomic(&root, Path::new("test.scn.ron"), &text).unwrap();
    let mut loaded = codec.load(&root, Path::new("test.scn.ron")).unwrap();
    let child = loaded
        .query::<(Entity, &Name)>()
        .iter(&loaded)
        .find(|(_, n)| n.as_str() == "Child")
        .unwrap()
        .0;
    let transform = loaded.get::<Transform>(child).unwrap();
    assert_eq!(transform.position, Vec3::new(0., 2., 0.));
    assert_eq!(
        loaded
            .get::<Name>(transform.parent.unwrap())
            .unwrap()
            .as_str(),
        "Parent"
    );
    assert_eq!(
        loaded.get::<PbrMaterial>(child).unwrap().roughness,
        Some(0.25)
    );
    assert!(loaded.get::<SceneOwned>(child).is_none());
    let scene = codec
        .decode(&fs::read_to_string(dir.path().join("test.scn.ron")).unwrap())
        .unwrap();
    let mut destination = World::new();
    let occupied = destination.spawn(Name::new("Existing")).id();
    let mut map = bevy_ecs::entity::EntityHashMap::default();
    scene
        .write_to_world_with(&mut destination, &mut map, &codec.registry)
        .unwrap();
    let (_, t) = destination
        .query::<(&Name, &Transform)>()
        .iter(&destination)
        .find(|(n, _)| n.as_str() == "Child")
        .unwrap();
    assert_ne!(t.parent, Some(occupied));
    assert_eq!(
        destination.get::<Name>(t.parent.unwrap()).unwrap().as_str(),
        "Parent"
    );
}

#[test]
fn scene_validation_rejects_cycles_external_references_and_unsupported_types() {
    let codec = SceneCodec::new();
    let mut world = fixture(&codec);
    let parent = world
        .query::<(Entity, &Name)>()
        .iter(&world)
        .find(|(_, n)| n.as_str() == "Parent")
        .unwrap()
        .0;
    let child = world
        .query::<(Entity, &Name)>()
        .iter(&world)
        .find(|(_, n)| n.as_str() == "Child")
        .unwrap()
        .0;
    world.get_mut::<Transform>(parent).unwrap().parent = Some(child);
    assert!(
        validate(&mut world)
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    world.get_mut::<Transform>(parent).unwrap().parent = None;
    let text = encode(&codec, &mut world);
    let mut scene = codec.decode(&text).unwrap();
    let phantom = world.spawn_empty().id();
    for e in &mut scene.entities {
        for c in &mut e.components {
            if c.represents::<Transform>() {
                use bevy_reflect::FromReflect;
                let mut t = Transform::from_reflect(c.as_partial_reflect()).unwrap();
                if t.parent.is_some() {
                    t.parent = Some(phantom);
                    *c = Box::new(t);
                }
            }
        }
    }
    assert!(
        codec
            .instantiate(&scene)
            .unwrap_err()
            .to_string()
            .contains("outside scene")
    );
    let unknown = text.replace(
        "lentille_render::transform::Transform",
        "other::UnknownComponent",
    );
    assert!(codec.decode(&unknown).is_err());
    #[derive(Component, bevy_reflect::Reflect)]
    #[reflect(Component)]
    struct RuntimeProbe {
        value: u32,
    }
    codec.registry.write().register::<RuntimeProbe>();
    let mut runtime_scene = codec.decode(&text).unwrap();
    runtime_scene.entities[0]
        .components
        .push(Box::new(RuntimeProbe { value: 7 }));
    let forbidden = codec.encode(&runtime_scene).unwrap();
    assert!(
        codec
            .decode(&forbidden)
            .err()
            .expect("runtime components must be rejected")
            .to_string()
            .contains("not persistent")
    );
    let version = text.replacen("version: 1", "version: 999", 1);
    assert!(
        codec
            .decode(&version)
            .err()
            .expect("unsupported version must fail")
            .to_string()
            .contains("version")
    );
}

#[test]
fn parent_rotation_scale_and_reparenting_produce_current_world_positions() {
    use lentille_render::transform::compute_world_transforms;
    use std::collections::HashMap;
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    let child = world.spawn_empty().id();
    let mut local = HashMap::from([
        (
            parent,
            Transform::new()
                .position(Vec3::new(10., 0., 0.))
                .rotation(Quat::from_angle_z(Deg(90.)))
                .scale(2.),
        ),
        (
            child,
            Transform {
                parent: Some(parent),
                position: Vec3::new(1., 0., 0.),
                ..Default::default()
            },
        ),
    ]);
    let result = compute_world_transforms(&local).unwrap();
    assert!((result[&child].position - Vec3::new(10., 2., 0.)).magnitude() < 0.0001);
    local.get_mut(&child).unwrap().parent = None;
    assert_eq!(
        compute_world_transforms(&local).unwrap()[&child].position,
        Vec3::new(1., 0., 0.)
    );
}

#[test]
fn browser_filters_sorts_and_restricts_real_filesystem_entries() {
    let dir = tempfile::tempdir().unwrap();
    let root = AssetRoot(dir.path().into());
    for name in ["empty", "nested", ".cache"] {
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    for name in [
        "z.scn.ron",
        "A.SCN.RON",
        "theme.ron",
        "scene.bsn",
        "mesh.glb",
        ".saving.scn.ron",
        "scene.scn.ron.tmp",
    ] {
        fs::write(dir.path().join(name), "").unwrap();
    }
    let entries = scan_scene_directory(&root, Path::new("")).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|e| e.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["empty", "nested", "A.SCN.RON", "z.scn.ron"]
    );
    assert_eq!(entries[0].kind, BrowserEntryKind::Folder);
    assert_eq!(entries[2].kind, BrowserEntryKind::Scene);
    assert!(
        scan_scene_directory(&root, Path::new("empty"))
            .unwrap()
            .is_empty()
    );
    assert!(scan_scene_directory(&root, Path::new("../")).is_err());
    assert!(root.resolve_path(Path::new("C:/outside")).is_err());
    fs::remove_dir(dir.path().join("nested")).unwrap();
    assert!(scan_scene_directory(&root, Path::new("nested")).is_err());
}

#[test]
fn asset_root_and_browser_reject_directory_links() {
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    let outside = directory.path().join("outside");
    fs::create_dir(&assets).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("hidden.scn.ron"), "outside").unwrap();
    let link = assets.join("linked");
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction creation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let root = AssetRoot(assets);
    assert!(
        scan_scene_directory(&root, Path::new(""))
            .unwrap()
            .is_empty()
    );
    assert!(
        root.resolve_path(Path::new("linked/hidden.scn.ron"))
            .is_err()
    );
    assert!(root.save_path(Path::new("linked/new.scn.ron")).is_err());
    assert_eq!(
        fs::read_to_string(outside.join("hidden.scn.ron")).unwrap(),
        "outside"
    );
}

#[test]
fn saving_replaces_complete_file_and_invalid_save_preserves_old_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = AssetRoot(dir.path().into());
    save_atomic(&root, Path::new("existing.scn.ron"), "original").unwrap();
    save_atomic(&root, Path::new("existing.scn.ron"), "replacement").unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.scn.ron")).unwrap(),
        "replacement"
    );
    assert!(save_atomic(&root, Path::new("existing.scn.ron/child.scn.ron"), "bad").is_err());
    assert!(save_atomic(&root, Path::new("../escape.scn.ron"), "bad").is_err());
    assert_eq!(
        fs::read_to_string(dir.path().join("existing.scn.ron")).unwrap(),
        "replacement"
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn scene_controller_protects_edits_and_preserves_state_on_io_failure() {
    use wgpu_pbr::scene::controller::{
        SceneController, SceneRequest, UnsavedDecision, sys_scene_requests,
    };
    let codec = SceneCodec::new();
    let mut world = fixture(&codec);
    let dir = tempfile::tempdir().unwrap();
    world.insert_resource(AssetRoot(dir.path().into()));
    world.insert_resource(codec);
    // These save / preflight-error paths do not touch the GPU.
    world.insert_resource(lentille_render::RenderReady);
    world.insert_resource(SceneController::default());
    let ids: Vec<_> = world.query::<Entity>().iter(&world).collect();
    for entity in ids {
        world.entity_mut(entity).insert(SceneOwned);
    }
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::SaveAs("current.scn.ron".into()));
    sys_scene_requests(&mut world);
    assert!(world.resource::<SceneController>().error.is_none());
    let saved = fs::read(dir.path().join("current.scn.ron")).unwrap();
    let entity = world
        .query_filtered::<Entity, With<PbrMaterial>>()
        .iter(&world)
        .next()
        .unwrap();
    world.get_mut::<PbrMaterial>(entity).unwrap().roughness = Some(0.75);
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::Open("missing.scn.ron".into()));
    sys_scene_requests(&mut world);
    assert!(world.resource::<SceneController>().pending_open.is_some());
    world
        .resource_mut::<SceneController>()
        .resolve_unsaved(UnsavedDecision::Cancel);
    assert!(world.resource::<SceneController>().dirty);
    assert_eq!(
        world.get::<PbrMaterial>(entity).unwrap().roughness,
        Some(0.75)
    );
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::SaveAs(
            "missing-parent/current.scn.ron".into(),
        ));
    sys_scene_requests(&mut world);
    assert!(world.resource::<SceneController>().error.is_some());
    assert!(world.resource::<SceneController>().dirty);
    assert_eq!(fs::read(dir.path().join("current.scn.ron")).unwrap(), saved);
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::Open("missing.scn.ron".into()));
    sys_scene_requests(&mut world);
    world
        .resource_mut::<SceneController>()
        .resolve_unsaved(UnsavedDecision::Discard);
    sys_scene_requests(&mut world);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while world.resource::<SceneController>().loading.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "CPU scene failure timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
        sys_scene_requests(&mut world);
    }
    let state = world.resource::<SceneController>();
    assert!(state.error.as_ref().unwrap().contains("missing.scn.ron"));
    assert_eq!(state.path.as_deref(), Some(Path::new("current.scn.ron")));
    assert!(state.dirty);
    assert_eq!(
        world.get::<PbrMaterial>(entity).unwrap().roughness,
        Some(0.75)
    );
}

#[test]
fn cancelled_cpu_load_cannot_replace_the_scene_or_contaminate_the_next_result() {
    use wgpu_pbr::scene::{
        SceneController, SceneLoadSettings, SceneRequest, controller::sys_scene_requests,
    };
    let codec = SceneCodec::new();
    let mut world = fixture(&codec);
    let directory = tempfile::tempdir().unwrap();
    let root = AssetRoot(directory.path().into());
    save_atomic(
        &root,
        Path::new("candidate.scn.ron"),
        &encode(&codec, &mut world),
    )
    .unwrap();
    let original: Vec<_> = world.query::<Entity>().iter(&world).collect();
    for e in &original {
        world.entity_mut(*e).insert(SceneOwned);
    }
    world.insert_resource(codec);
    world.insert_resource(root);
    world.insert_resource(lentille_render::RenderReady);
    world.insert_resource(SceneController::default());
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::Open("candidate.scn.ron".into()));
    sys_scene_requests(&mut world);
    let first_id = world
        .resource::<SceneController>()
        .loading
        .as_ref()
        .unwrap()
        .id;
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::SaveAs("blocked.scn.ron".into()));
    assert!(
        world.resource::<SceneController>().request.is_none(),
        "Busy controller must reject external saves"
    );
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::CancelLoad);
    sys_scene_requests(&mut world);
    assert!(!world.resource::<SceneController>().read_only());
    assert!(world.resource::<SceneController>().path.is_none());
    // A valid next request with an impossibly small CPU budget must fail before GPU access.
    world.insert_resource(SceneLoadSettings {
        max_cpu_bytes: 1,
        ..Default::default()
    });
    world
        .resource_mut::<SceneController>()
        .request(SceneRequest::Open("candidate.scn.ron".into()));
    sys_scene_requests(&mut world);
    assert!(
        world
            .resource::<SceneController>()
            .loading
            .as_ref()
            .unwrap()
            .id
            > first_id
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while world.resource::<SceneController>().loading.is_some() {
        assert!(
            std::time::Instant::now() < deadline,
            "Cancelled worker failed to release its slot"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
        sys_scene_requests(&mut world);
    }
    assert!(
        world
            .resource::<SceneController>()
            .error
            .as_ref()
            .unwrap()
            .contains("budget")
    );
    assert!(world.resource::<SceneController>().path.is_none());
    assert!(original.iter().all(|e| world.get_entity(*e).is_ok()));
    assert!(!directory.path().join("blocked.scn.ron").exists());
}

#[test]
fn repository_scenes_load_without_gpu_and_minimal_has_no_external_models() {
    let codec = SceneCodec::new();
    let root = AssetRoot::default();
    let mut minimal = codec
        .load(&root, Path::new("scenes/minimal.scn.ron"))
        .unwrap();
    assert_eq!(minimal.query::<&Camera>().iter(&minimal).count(), 1);
    assert!(
        minimal
            .query::<&MeshSource>()
            .iter(&minimal)
            .all(|s| matches!(s, MeshSource::Plane { .. }))
    );
    let mut demo = codec.load(&root, Path::new("scenes/demo.scn.ron")).unwrap();
    assert!(
        demo.query::<&MeshSource>()
            .iter(&demo)
            .any(|s| matches!(s, MeshSource::Asset(_)))
    );
}
