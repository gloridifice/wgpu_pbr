//! Generate self-contained white-furnace assets without a GPU or external models.
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
use serde_json::json;
use wgpu_pbr::scene::codec::{SceneCodec, SceneMetadata, save_atomic, validate};

fn sphere() -> Result<()> {
    // Shared unit UV sphere, outward CCW winding; omit degenerate pole triangles.
    let (slices, stacks) = (96u32, 48u32);
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    for y in 0..=stacks {
        let theta = std::f32::consts::PI * y as f32 / stacks as f32;
        for x in 0..=slices {
            let phi = std::f32::consts::TAU * x as f32 / slices as f32;
            positions.push([
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ]);
        }
    }
    for y in 0..stacks {
        for x in 0..slices {
            let a = y * (slices + 1) + x;
            let b = a + slices + 1;
            if y > 0 {
                indices.extend([a, a + 1, b]);
            }
            if y + 1 < stacks {
                indices.extend([a + 1, b + 1, b]);
            }
        }
    }
    let vertex_bytes = positions.len() * 12;
    let mut bytes = Vec::new();
    for p in &positions {
        for v in p {
            bytes.extend(v.to_le_bytes());
        }
    }
    for i in &indices {
        bytes.extend(i.to_le_bytes());
    }
    std::fs::write("assets/models/white_furnace/sphere.bin", &bytes)?;
    let gltf = json!({
        "asset": {"version": "2.0", "generator": "generate_white_furnace"},
        "buffers": [{"uri": "sphere.bin", "byteLength": bytes.len()}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": vertex_bytes, "target": 34962},
            {"buffer": 0, "byteOffset": vertex_bytes, "byteLength": indices.len() * 4, "target": 34963}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": positions.len(), "type": "VEC3", "min": [-1,-1,-1], "max": [1,1,1]},
            {"bufferView": 1, "componentType": 5125, "count": indices.len(), "type": "SCALAR"}
        ],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 0}, "indices": 1}]}],
        "nodes": [{"mesh": 0}], "scenes": [{"nodes": [0]}], "scene": 0
    });
    std::fs::write(
        "assets/models/white_furnace/sphere.gltf",
        serde_json::to_string_pretty(&gltf)?,
    )?;
    // Exercise the actual importer, not just the glTF serializer.
    import_model(
        &AssetRoot::default(),
        &AssetPath::new("models/white_furnace/sphere.gltf"),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    std::fs::create_dir_all("assets/models/white_furnace")?;
    std::fs::create_dir_all("assets/textures/white_furnace")?;
    std::fs::create_dir_all("assets/scenes")?;
    sphere()?;
    // White is exactly 1 in both linear and sRGB, avoiding transfer-function ambiguity.
    // Enough latitude samples to keep SH quadrature error small.
    image::RgbaImage::from_pixel(512, 256, image::Rgba([255u8; 4]))
        .save("assets/textures/white_furnace/white.png")?;

    let codec = SceneCodec::new();
    let mut world = World::new();
    world.insert_resource(codec.registry.clone());
    let camera = world
        .spawn((
            Name::new("Camera"),
            Camera {
                fovy: 45.,
                ..Camera::new(1.)
            },
            RenderTargetConfig::Viewport,
            CsmConfig {
                level_count: 1,
                texture_size: 256,
                linear_log_factor: 0.5,
                shadow_near: 1.,
                shadow_far: 40.,
            },
            Transform::from_position(Vec3::new(0., 0., 14.)),
        ))
        .id();
    world.spawn((
        SceneMetadata {
            name: "White Furnace — roughness left to right".into(),
            active_camera: camera,
        },
        Environment::Image {
            path: AssetPath::new("textures/white_furnace/white.png"),
            size: 64,
            levels: 7,
            samples: 1024,
        },
    ));
    // The scene contract requires one sun. Zero intensity removes direct lighting.
    world.spawn((
        Name::new("Sun (disabled)"),
        ParallelLight {
            intensity: 0.,
            ..Default::default()
        },
        Transform::new(),
    ));
    for (row, (label, metallic, reflectance)) in [
        ("Metal F0=1", 1., 0.5),
        ("Dielectric F0=0.04", 0., 0.5),
        ("Diffuse control F0=0", 0., 0.),
    ]
    .into_iter()
    .enumerate()
    {
        for (col, roughness) in [0., 0.1, 0.25, 0.5, 0.75, 1.].into_iter().enumerate() {
            world.spawn((
                Name::new(format!("{label} / roughness={roughness:.2}")),
                MeshSource::Asset(AssetRef::new(
                    "models/white_furnace/sphere.gltf",
                    SubAsset::Mesh(0),
                )),
                Transform::from_position(Vec3::new(
                    (col as f32 - 2.5) * 2.6,
                    (1. - row as f32) * 2.6,
                    0.,
                )),
                MainPassObject,
                Pickable,
                PbrMaterial {
                    color: Some(Color::new(1., 1., 1., 1.)),
                    roughness: Some(roughness),
                    metallic: Some(metallic),
                    reflectance: Some(reflectance),
                    ..Default::default()
                },
            ));
        }
    }
    validate(&mut world)?;
    let ids: Vec<_> = world.query::<Entity>().iter(&world).collect();
    let scene = codec.capture(&world, ids.into_iter())?;
    let text = codec.encode(&scene)?;
    let mut restored = codec.instantiate(&codec.decode(&text)?)?;
    validate(&mut restored)?;
    save_atomic(
        &AssetRoot::default(),
        std::path::Path::new("scenes/white_furnace.scn.ron"),
        &text,
    )?;
    println!("Wrote scenes/white_furnace.scn.ron and its sphere/white environment assets");
    Ok(())
}
