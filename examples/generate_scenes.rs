use anyhow::Result;
use bevy_ecs::prelude::*;
use lentille_render::{
    asset::{AssetRef, AssetRoot, MeshSource, SubAsset, load::import_model},
    camera::RenderTargetConfig,
    picking::resources::Pickable,
    prelude::*,
    shadow::csm::CsmConfig,
    skybox::Environment,
};
use wgpu_pbr::scene::codec::{SceneCodec, SceneMetadata, save_atomic, validate};

fn base(codec: &SceneCodec, name: &str) -> World {
    let mut world = World::new();
    world.insert_resource(codec.registry.clone());
    let camera = world
        .spawn((
            Name::new("Camera"),
            Camera {
                fovy: 50.,
                ..Camera::new(1.)
            },
            RenderTargetConfig::Viewport,
            CsmConfig {
                level_count: 4,
                texture_size: 2048,
                linear_log_factor: 0.5,
                shadow_near: 1.,
                shadow_far: 80.,
            },
            Transform::new()
                .position(Vec3::new(0., 9., 17.))
                .rotation(Quat::from_angle_x(Deg(-25.))),
        ))
        .id();
    world.spawn((
        SceneMetadata {
            name: name.into(),
            active_camera: camera,
        },
        Environment::Default,
    ));
    world.spawn((
        Name::new("Sun"),
        ParallelLight::default(),
        Transform::new()
            .position(Vec3::new(0., 4., 5.))
            .rotation(Quat::from_angle_x(Deg(-45.))),
    ));
    world.spawn((
        Name::new("Ground"),
        Transform::from_position(Vec3::new(0., -0.1, 0.)),
        MeshSource::Plane { size: 24. },
        MainPassObject,
        CastShadow,
        Pickable,
        PbrMaterial {
            color: Some(Color::new(0.55, 0.6, 0.62, 1.)),
            roughness: Some(0.8),
            metallic: Some(0.05),
            ..Default::default()
        },
    ));
    world
}
fn write(codec: &SceneCodec, mut world: World, path: &str) -> Result<()> {
    validate(&mut world)?;
    let ids: Vec<_> = world.query::<Entity>().iter(&world).collect();
    let scene = codec.capture(&world, ids.into_iter())?;
    let text = codec.encode(&scene)?;
    codec.instantiate(&codec.decode(&text)?)?;
    save_atomic(&AssetRoot::default(), std::path::Path::new(path), &text)
}
fn main() -> Result<()> {
    std::fs::create_dir_all("assets/scenes")?;
    let codec = SceneCodec::new();
    let mut minimal = base(&codec, "Material Study");
    for (x, color, roughness) in [
        (-4., Color::new(0.8, 0.12, 0.1, 1.), 0.1),
        (0., Color::new(0.12, 0.65, 0.35, 1.), 0.5),
        (4., Color::new(0.1, 0.3, 0.85, 1.), 0.9),
    ] {
        minimal.spawn((
            Name::new("Material sample"),
            MeshSource::Plane { size: 3. },
            Transform::new()
                .position(Vec3::new(x, 2., 0.))
                .rotation(Quat::from_angle_x(Deg(65.))),
            MainPassObject,
            CastShadow,
            Pickable,
            PbrMaterial {
                color: Some(color),
                roughness: Some(roughness),
                metallic: Some(0.3),
                ..Default::default()
            },
        ));
    }
    write(&codec, minimal, "scenes/minimal.scn.ron")?;
    let mut demo = base(&codec, "PBR Demo");
    for (name, path, transform) in [
        (
            "Dragon",
            "models/stanford_dragon_pbr.glb",
            Transform::new().scale(0.05),
        ),
        (
            "Landscape",
            "models/rough_mountaintop_landscape.glb",
            Transform::new()
                .position(Vec3::new(0., -375., 0.))
                .scale(0.5),
        ),
        (
            "Vending Machine",
            "models/vending_machine2k_retro-futuristic/scene.gltf",
            Transform::new()
                .rotation(Quat::from_angle_x(Deg(-90.)))
                .scale(2.),
        ),
    ] {
        let model = import_model(&AssetRoot::default(), &AssetPath::new(path))?;
        let parent = demo.spawn((Name::new(name), transform)).id();
        for index in 0..model.meshes.len() {
            demo.spawn((
                Name::new(format!("{name} mesh {index}")),
                Transform {
                    parent: Some(parent),
                    ..Default::default()
                },
                MeshSource::Asset(AssetRef::new(path, SubAsset::Mesh(index))),
                MainPassObject,
                CastShadow,
                Pickable,
            ));
        }
    }
    for i in 0..20 {
        demo.spawn((
            Name::new(format!("Point light {i}")),
            PointLight {
                color: Color::new(
                    0.2 + (i % 3) as f32 * 0.3,
                    0.5,
                    0.8 - (i % 4) as f32 * 0.15,
                    1.,
                ),
                intensity: 1. + (i % 5) as f32 * 0.2,
                ..Default::default()
            },
            Transform::from_position(Vec3::new(
                (i % 5) as f32 * 2.,
                1. + (i % 3) as f32,
                (i / 5) as f32 * 2.,
            )),
        ));
    }
    let root = demo
        .query::<(Entity, &SceneMetadata)>()
        .iter(&demo)
        .next()
        .unwrap()
        .0;
    demo.entity_mut(root).insert(Environment::Image {
        path: AssetPath::new("textures/skybox/sky_110_2k.png"),
        size: 512,
        levels: 5,
        samples: 1145,
    });
    write(&codec, demo, "scenes/demo.scn.ron")?;
    println!("Wrote minimal.scn.ron and demo.scn.ron");
    Ok(())
}
