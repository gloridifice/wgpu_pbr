use super::*;

#[test]
fn cpu_plan_shares_pixels_but_preserves_texture_color_space_and_material_overrides() {
    let directory = tempfile::tempdir().unwrap();
    image::RgbaImage::from_pixel(2, 1, image::Rgba([17, 31, 63, 255]))
        .save(directory.path().join("shared.png"))
        .unwrap();
    let reference = AssetRef::new("./shared.png", SubAsset::File);
    let material = PbrMaterial {
        base_color_texture: TextureOverride::Set(reference.clone()),
        normal_texture: TextureOverride::Set(AssetRef::new("shared.png", SubAsset::File)),
        roughness: Some(0.3),
        ..Default::default()
    };
    let requests = vec![(MeshSource::Plane { size: 2.0 }, Some(material.clone())); 2];
    let assets = CpuAssets::load(
        AssetRoot(directory.path().into()),
        &requests,
        1024 * 1024,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(assets.meshes.len(), 1, "Instances share geometry");
    assert_eq!(assets.materials.len(), 1);
    assert_eq!(assets.materials[0].roughness, Some(0.3));
    assert_eq!(
        assets.textures.len(),
        2,
        "sRGB and linear uses are separate GPU textures"
    );
    assert_ne!(assets.textures[0].0.1, assets.textures[1].0.1);
    assert!(
        Arc::ptr_eq(&assets.textures[0].1, &assets.textures[1].1),
        "Path aliases and color-space variants share decoded pixels"
    );
    assert_eq!(
        assets.textures[0].1.pixels,
        [17, 31, 63, 255, 17, 31, 63, 255]
    );
    assert_eq!(assets.gpu_bytes, mesh_bytes(&assets.meshes[0].1) + 16);
    assert!(CpuAssets::load(AssetRoot(directory.path().into()), &requests, 1, |_| Ok(())).is_err());
    assert!(
        CpuAssets::load(
            AssetRoot(directory.path().into()),
            &requests,
            1024 * 1024,
            |_| anyhow::bail!("cancelled")
        )
        .is_err()
    );
}
