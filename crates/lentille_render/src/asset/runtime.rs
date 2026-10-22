use super::{
    AssetRef, AssetRoot, MeshAsset, SubAsset, TextureAsset,
    load::{import_model, load_image},
};
use crate::{
    deferred_rendering::DeferredComputePipeline,
    material::pbr::{ResolvedMaterial, TextureOverride, UploadedPbrMaterial},
    prelude::*,
};
use anyhow::{Context, Result, ensure};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;
use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

mod loading;
pub use loading::{AssetUpload, CpuAssets, FrameBudget, RgbaImageData};

#[derive(Component, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub enum MeshSource {
    Asset(AssetRef<MeshAsset>),
    Plane { size: f32 },
}

#[derive(Debug)]
pub struct GpuHandle<T> {
    scope: u64,
    index: usize,
    marker: PhantomData<fn() -> T>,
}
impl<T> Copy for GpuHandle<T> {}
impl<T> Clone for GpuHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> PartialEq for GpuHandle<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.scope == rhs.scope && self.index == rhs.index
    }
}
impl<T> Eq for GpuHandle<T> {}

#[derive(Resource)]
pub struct SceneAssetCache {
    scope: u64,
    pub root: AssetRoot,
    models: HashMap<AssetPath, Arc<Model>>,
    mesh_keys: HashMap<String, usize>,
    meshes: Vec<Arc<UploadedMesh>>,
    mesh_data: Vec<Arc<Mesh>>,
    textures:
        HashMap<(AssetRef<TextureAsset>, bool), Arc<UploadedImage<Dim2D, SampleFloatFilterable>>>,
    materials: Vec<(PbrMaterial, Arc<UploadedPbrMaterial>)>,
    preloaded: bool,
}
impl Default for SceneAssetCache {
    fn default() -> Self {
        Self::new(AssetRoot::default())
    }
}
impl SceneAssetCache {
    pub fn new(root: AssetRoot) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            scope: NEXT.fetch_add(1, Ordering::Relaxed),
            root,
            models: HashMap::new(),
            mesh_keys: HashMap::new(),
            meshes: Vec::new(),
            mesh_data: Vec::new(),
            textures: HashMap::new(),
            materials: Vec::new(),
            preloaded: false,
        }
    }
    pub fn model(&mut self, path: &AssetPath) -> Result<Arc<Model>> {
        let path = normalized(path)?;
        if let Some(v) = self.models.get(&path) {
            return Ok(v.clone());
        }
        ensure!(
            !self.preloaded,
            "Model missing from CPU load plan: {path:?}"
        );
        let model = Arc::new(import_model(&self.root, &path)?);
        self.models.insert(path, model.clone());
        Ok(model)
    }
    pub fn mesh(&self, handle: GpuHandle<UploadedMesh>) -> Option<&UploadedMesh> {
        (handle.scope == self.scope)
            .then(|| self.meshes.get(handle.index))
            .flatten()
            .map(Arc::as_ref)
    }
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.models.len(), self.meshes.len(), self.textures.len())
    }
    pub fn estimated_bytes(&self) -> (u64, u64) {
        let cpu = self
            .models
            .values()
            .map(|m| loading::model_bytes(m))
            .sum::<u64>()
            + self
                .mesh_data
                .iter()
                .map(|m| loading::mesh_bytes(m))
                .sum::<u64>();
        let gpu = self
            .mesh_data
            .iter()
            .map(|m| loading::mesh_bytes(m))
            .sum::<u64>()
            + self
                .textures
                .values()
                .map(|t| u64::from(t.texture.width()) * u64::from(t.texture.height()) * 4)
                .sum::<u64>();
        (cpu, gpu)
    }
    pub fn request_mesh(
        &mut self,
        source: &MeshSource,
        world: &World,
    ) -> Result<GpuHandle<UploadedMesh>> {
        let (key, data) = match source {
            MeshSource::Asset(reference) => {
                let path = normalized(&reference.path)?;
                let key = format!("{path:?}/{:?}", reference.label);
                if let Some(&index) = self.mesh_keys.get(&key) {
                    return Ok(GpuHandle {
                        scope: self.scope,
                        index,
                        marker: PhantomData,
                    });
                }
                ensure!(
                    !self.preloaded,
                    "Mesh missing from CPU upload plan: {source:?}"
                );
                let SubAsset::Mesh(index) = reference.label else {
                    anyhow::bail!("Mesh reference requires Mesh(index)");
                };
                let model = self.model(&path)?;
                (
                    key,
                    Arc::new(
                        model
                            .meshes
                            .get(index)
                            .with_context(|| format!("Missing mesh {index} in {path:?}"))?
                            .clone(),
                    ),
                )
            }
            MeshSource::Plane { size } => {
                ensure!(size.is_finite() && *size > 0., "Invalid plane size");
                let key = format!("plane/{}", size.to_bits());
                if let Some(&index) = self.mesh_keys.get(&key) {
                    return Ok(GpuHandle {
                        scope: self.scope,
                        index,
                        marker: PhantomData,
                    });
                }
                ensure!(!self.preloaded, "Plane missing from CPU upload plan");
                (key, Arc::new(Mesh::plane(*size)))
            }
        };
        let rs = world.resource::<RenderState>();
        ensure!(
            data.vertices.len() * std::mem::size_of::<Vertex>()
                <= rs.device.limits().max_buffer_size as usize
                && data.indices.len() * 4 <= rs.device.limits().max_buffer_size as usize,
            "Mesh exceeds GPU buffer limits"
        );
        let uploaded = Arc::new(data.upload(&rs.device));
        let index = self.meshes.len();
        self.meshes.push(uploaded);
        self.mesh_data.push(data);
        self.mesh_keys.insert(key, index);
        Ok(GpuHandle {
            scope: self.scope,
            index,
            marker: PhantomData,
        })
    }
    fn texture(
        &mut self,
        source: &AssetRef<TextureAsset>,
        srgb: bool,
        world: &World,
    ) -> Result<Arc<UploadedImage<Dim2D, SampleFloatFilterable>>> {
        let mut source = source.clone();
        source.path = normalized(&source.path)?;
        let key = (source.clone(), srgb);
        if let Some(v) = self.textures.get(&key) {
            return Ok(v.clone());
        }
        ensure!(
            !self.preloaded,
            "Texture missing from CPU load plan: {source:?}"
        );
        let data = match source.label {
            SubAsset::File => load_image(&self.root.resolve(&source.path)?)?,
            SubAsset::Image(index) => self
                .model(&source.path)?
                .images
                .get(index)
                .with_context(|| format!("Missing image {index} in {:?}", source.path))?
                .clone(),
            _ => anyhow::bail!("Texture reference must select File or Image"),
        };
        let rs = world.resource::<RenderState>();
        let format = if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        };
        let image = Arc::new(UploadedImage::load_from_data(
            &data, &rs.device, &rs.queue, format,
        )?);
        self.textures.insert(key, image.clone());
        Ok(image)
    }
    fn material(
        &mut self,
        description: PbrMaterial,
        world: &World,
    ) -> Result<Arc<UploadedPbrMaterial>> {
        if let Some((_, v)) = self.materials.iter().find(|(key, _)| key == &description) {
            return Ok(v.clone());
        }
        let mut textures = std::array::from_fn(|_| None);
        for (i, source) in [
            &description.base_color_texture,
            &description.normal_texture,
            &description.metallic_roughness_ao_texture,
            &description.emission_texture,
        ]
        .into_iter()
        .enumerate()
        {
            if let TextureOverride::Set(source) = source {
                textures[i] = Some(self.texture(source, i == 0 || i == 3, world)?);
            }
        }
        let rs = world.resource::<RenderState>();
        let material = Arc::new(UploadedPbrMaterial::new(
            &rs.device,
            world.resource::<PbrMaterialBindGroupLayout>(),
            &world.resource::<WhiteTexture>().0,
            &world.resource::<NormalDefaultTexture>().0,
            &world.resource::<DefaultMaterialSampler>().0,
            world.resource::<DeferredComputePipeline>().pipeline.clone(),
            &ResolvedMaterial {
                description: description.clone(),
                textures,
            },
        ));
        self.materials.push((description, material.clone()));
        Ok(material)
    }
    pub fn prepare(
        &mut self,
        source: &MeshSource,
        overrides: Option<&PbrMaterial>,
        world: &World,
    ) -> Result<MeshRenderer> {
        let mesh = self.request_mesh(source, world)?;
        let data = self.mesh_data[mesh.index].clone();
        let mut materials = Vec::new();
        for primitive in &data.primitives {
            let base = primitive.material.clone().unwrap_or_default();
            let desc = overrides.map(|v| v.merged(&base)).unwrap_or(base);
            materials.push(self.material(desc, world)?);
        }
        Ok(MeshRenderer::new(mesh, materials, world))
    }
}
fn normalized(path: &AssetPath) -> Result<AssetPath> {
    let AssetPath::Assets(s) = path;
    Ok(AssetPath::Assets(
        AssetRoot::relative(std::path::Path::new(s))?
            .to_str()
            .context("Asset path is not UTF-8")?
            .replace('\\', "/"),
    ))
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct PreparationState {
    pub source: MeshSource,
    pub material: Option<PbrMaterial>,
    pub error: Option<String>,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetPreparationSet;

pub fn sys_prepare_assets(world: &mut World) {
    if !world.contains_resource::<crate::RenderReady>() {
        return;
    }
    let missing_world: Vec<_> = world
        .query_filtered::<Entity, (With<Transform>, Without<WorldTransform>)>()
        .iter(world)
        .collect();
    for entity in missing_world {
        world.entity_mut(entity).insert(WorldTransform::default());
    }
    let missing_camera_state: Vec<_> = world
        .query_filtered::<Entity, (With<Camera>, Without<crate::camera::CameraState>)>()
        .iter(world)
        .collect();
    for entity in missing_camera_state {
        world
            .entity_mut(entity)
            .insert(crate::camera::CameraState::default());
    }
    let orphan_cameras: Vec<_> = world
        .query_filtered::<Entity, (
            Or<(
                With<crate::camera::RenderTarget>,
                With<crate::camera::CameraBuffer>,
            )>,
            Or<(Without<Camera>, Without<crate::camera::RenderTargetConfig>)>,
        )>()
        .iter(world)
        .collect();
    for entity in orphan_cameras {
        world.entity_mut(entity).remove::<(
            crate::camera::RenderTarget,
            crate::camera::RenderTargetSize,
            crate::camera::CameraBuffer,
            crate::camera::CameraState,
            crate::shadow::csm::CascadeShadowMapping,
            crate::deferred_rendering::write_g_buffer_pipeline::GBufferTextureBindGroup,
        )>();
    }
    let removed: Vec<_> = world
        .query_filtered::<Entity, (
            Or<(With<MeshRenderer>, With<PreparationState>)>,
            Or<(Without<MeshSource>, Without<Transform>)>,
        )>()
        .iter(world)
        .collect();
    for e in removed {
        world
            .entity_mut(e)
            .remove::<(MeshRenderer, PreparationState)>();
    }
    let requests: Vec<_> = world
        .query_filtered::<(
            Entity,
            &MeshSource,
            Option<&PbrMaterial>,
            Option<&PreparationState>,
            Option<&MeshRenderer>,
        ), With<Transform>>()
        .iter(world)
        .filter(|(_, s, m, state, renderer)| {
            state.is_none_or(|v| {
                v.source != **s
                    || v.material.as_ref() != *m
                    || (v.error.is_none() && renderer.is_none())
            })
        })
        .map(|(e, s, m, _, _)| (e, s.clone(), m.cloned()))
        .collect();
    world.resource_scope(|world, mut cache: Mut<SceneAssetCache>| {
        for (e, source, material) in requests {
            world.entity_mut(e).remove::<MeshRenderer>();
            let error = match cache.prepare(&source, material.as_ref(), world) {
                Ok(renderer) => {
                    world.entity_mut(e).insert(renderer);
                    None
                }
                Err(error) => {
                    bevy_log::error!("Entity {e:?} asset {source:?}: {error:#}");
                    Some(format!("{error:#}"))
                }
            };
            world.entity_mut(e).insert(PreparationState {
                source,
                material,
                error,
            });
        }
    });
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    // Owns the GPU preparation lifecycle contract, not the cache implementation.
    #[test]
    #[ignore = "requires a GPU and engine assets; run explicitly with one test thread"]
    fn gpu_assets_share_geometry_rebuild_instances_retry_and_release_scope() {
        let original = std::env::current_dir().unwrap();
        struct Restore(std::path::PathBuf);
        impl Drop for Restore {
            fn drop(&mut self) {
                let _ = std::env::set_current_dir(&self.0);
            }
        }
        let _restore = Restore(original);
        std::env::set_current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .unwrap();
        let mut app = bevy_app::App::new();
        app.add_plugins(crate::RenderPlugin);
        let graph = std::mem::replace(
            &mut *crate::resource::RENDER_RESOURCES_TO_ADD.lock().unwrap(),
            crate::resource::ResourceGraph::new(),
        );
        for initialize in graph {
            initialize(app.world_mut());
        }
        let world = app.world_mut();
        let dir = tempfile::tempdir().unwrap();
        world.insert_resource(SceneAssetCache::new(AssetRoot(dir.path().into())));
        world.insert_resource(crate::RenderReady);
        let source = MeshSource::Plane { size: 2.0 };
        let first = world.spawn((source.clone(), Transform::default())).id();
        let second = world.spawn((source, Transform::default())).id();
        sys_prepare_assets(world);
        let a = world.get::<MeshRenderer>(first).unwrap();
        let b = world.get::<MeshRenderer>(second).unwrap();
        let handle = a.mesh;
        assert!(a.mesh == b.mesh);
        assert!(!Arc::ptr_eq(&a.transform_buffer, &b.transform_buffer));
        let old_buffer = a.transform_buffer.clone();
        let cache = world.resource::<SceneAssetCache>();
        let weak_mesh = Arc::downgrade(&cache.meshes[handle.index]);
        assert_eq!(cache.meshes.len(), 1);
        world.entity_mut(first).remove::<MeshRenderer>();
        sys_prepare_assets(world);
        assert!(world.get::<MeshRenderer>(first).unwrap().mesh == handle);
        assert!(!Arc::ptr_eq(
            &world.get::<MeshRenderer>(first).unwrap().transform_buffer,
            &old_buffer
        ));
        world.entity_mut(first).remove::<MeshSource>();
        sys_prepare_assets(world);
        assert!(world.get::<MeshRenderer>(first).is_none());
        image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 255, 255]))
            .save(dir.path().join("shared.png"))
            .unwrap();
        let texture = AssetRef::new("shared.png", SubAsset::File);
        world.entity_mut(second).insert(PbrMaterial {
            base_color_texture: TextureOverride::Set(texture.clone()),
            normal_texture: TextureOverride::Set(texture),
            ..Default::default()
        });
        sys_prepare_assets(world);
        let cache = world.resource::<SceneAssetCache>();
        assert_eq!(cache.textures.len(), 2);
        assert!(
            cache
                .textures
                .values()
                .any(|v| v.texture.format() == TextureFormat::Rgba8UnormSrgb)
        );
        assert!(
            cache
                .textures
                .values()
                .any(|v| v.texture.format() == TextureFormat::Rgba8Unorm)
        );
        world.entity_mut(second).remove::<PbrMaterial>();
        sys_prepare_assets(world);
        assert!(world.get::<MeshRenderer>(second).unwrap().mesh == handle);
        world
            .entity_mut(second)
            .insert(MeshSource::Asset(AssetRef::new(
                "retry.gltf",
                SubAsset::Mesh(0),
            )));
        sys_prepare_assets(world);
        assert!(world.get::<MeshRenderer>(second).is_none());
        assert!(
            world
                .get::<PreparationState>(second)
                .unwrap()
                .error
                .is_some()
        );
        let points: [f32; 9] = [0., 0., 0., 1., 0., 0., 0., 1., 0.];
        std::fs::write(
            dir.path().join("mesh.bin"),
            points
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        std::fs::write(dir.path().join("retry.gltf"), r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"mesh.bin","byteLength":36}],"bufferViews":[{"buffer":0,"byteLength":36}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}]}"#).unwrap();
        sys_prepare_assets(world);
        assert!(
            world.get::<MeshRenderer>(second).is_none(),
            "failed revisions must not retry automatically"
        );
        world.entity_mut(second).remove::<PreparationState>();
        sys_prepare_assets(world);
        assert!(world.get::<MeshRenderer>(second).is_some());
        assert!(
            world
                .get::<PreparationState>(second)
                .unwrap()
                .error
                .is_none()
        );
        let stream_path = dir.path().join("stream.png");
        image::RgbaImage::from_pixel(513, 257, image::Rgba([17, 31, 63, 255]))
            .save(&stream_path)
            .unwrap();
        let description = PbrMaterial {
            base_color_texture: TextureOverride::Set(AssetRef::new("stream.png", SubAsset::File)),
            normal_texture: TextureOverride::Set(AssetRef::new("stream.png", SubAsset::File)),
            ..Default::default()
        };
        let source = MeshSource::Plane { size: 3.0 };
        let cpu = CpuAssets::load(
            AssetRoot(dir.path().into()),
            &[(source.clone(), Some(description.clone()))],
            16 * 1024 * 1024,
            |_| Ok(()),
        )
        .unwrap();
        std::fs::remove_file(stream_path).unwrap();
        let mut upload = cpu.into_upload();
        let mut batches = 0;
        loop {
            batches += 1;
            assert!(batches < 1000, "Partial row uploads must make progress");
            let mut budget = FrameBudget::new(std::time::Duration::from_millis(10), 65536);
            if upload.advance(world, &mut budget).unwrap() {
                break;
            }
        }
        assert!(batches > 1, "Textures must upload across budgeted steps");
        assert_eq!(upload.uploaded_bytes, upload.total_bytes);
        let mut loaded_cache = upload.finish().unwrap();
        assert_eq!(loaded_cache.counts(), (0, 1, 2));
        loaded_cache
            .prepare(&source, Some(&description), world)
            .unwrap();
        image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]))
            .save(dir.path().join("edited.png"))
            .unwrap();
        loaded_cache
            .prepare(
                &source,
                Some(&PbrMaterial {
                    base_color_texture: TextureOverride::Set(AssetRef::new(
                        "edited.png",
                        SubAsset::File,
                    )),
                    ..Default::default()
                }),
                world,
            )
            .expect("Committed caches must permit synchronous runtime asset edits");
        world.despawn(first);
        world.despawn(second);
        world.insert_resource(SceneAssetCache::default());
        assert!(world.resource::<SceneAssetCache>().mesh(handle).is_none());
        assert!(
            weak_mesh.upgrade().is_none(),
            "old scene scope must release geometry"
        );
    }
}
