use lentille_render::{
    asset::{AssetPath, AssetRoot, SubAsset, load::import_model},
    material::pbr::{PbrMaterial, TextureOverride},
    prelude::Color,
};
use std::fs;
#[test]
fn importer_offsets_primitive_indices_and_resolves_texture_source_not_texture_index() {
    let dir = tempfile::tempdir().unwrap();
    let positions: [f32; 9] = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
    fs::write(
        dir.path().join("mesh.bin"),
        positions
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]))
        .save(dir.path().join("red.png"))
        .unwrap();
    let doc = serde_json::json!({
        "asset":{"version":"2.0"},"buffers":[{"uri":"mesh.bin","byteLength":36}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}],
        "images":[{"uri":"red.png"}],"textures":[{"source":0},{"source":0}],
        "materials":[{"pbrMetallicRoughness":{"baseColorFactor":[1,0,0,1],"baseColorTexture":{"index":1}}},{"pbrMetallicRoughness":{"baseColorFactor":[0,1,0,1]}}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0},"material":0},{"attributes":{"POSITION":0},"material":1}]}]
    });
    fs::write(
        dir.path().join("mesh.gltf"),
        serde_json::to_vec(&doc).unwrap(),
    )
    .unwrap();
    let model = import_model(&AssetRoot(dir.path().into()), &AssetPath::new("mesh.gltf")).unwrap();
    assert_eq!(model.meshes[0].indices, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(model.meshes[0].vertices.len(), 6);
    let first = model.meshes[0].primitives[0].material.as_ref().unwrap();
    let TextureOverride::Set(texture) = &first.base_color_texture else {
        panic!("Missing texture reference");
    };
    assert_eq!(texture.label, SubAsset::Image(0));
    let overlay = PbrMaterial {
        roughness: Some(0.2),
        ..Default::default()
    };
    let red = overlay.merged(first);
    let green = overlay.merged(model.meshes[0].primitives[1].material.as_ref().unwrap());
    assert_eq!(red.color, Some(Color::RED));
    assert_eq!(green.color, Some(Color::GREEN));
    assert_eq!(red.roughness, Some(0.2));
    assert_eq!(red.base_color_texture, first.base_color_texture);
    let clear = PbrMaterial {
        base_color_texture: TextureOverride::Clear,
        ..Default::default()
    }
    .merged(first);
    assert_eq!(clear.base_color_texture, TextureOverride::Clear);
    fs::write(
        dir.path().join("mesh.gltf"),
        doc.to_string().replace("mesh.bin", "../escape.bin"),
    )
    .unwrap();
    let error = import_model(&AssetRoot(dir.path().into()), &AssetPath::new("mesh.gltf"))
        .err()
        .unwrap();
    assert!(error.to_string().contains("escapes root"));
}
