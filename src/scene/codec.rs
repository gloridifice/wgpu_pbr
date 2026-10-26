use crate::RotationObject;
use anyhow::{Context, Result, ensure};
use bevy_ecs::{
    entity::EntityHashMap,
    prelude::*,
    reflect::{AppTypeRegistry, ReflectComponent},
};
use bevy_reflect::{FromReflect, Reflect};
use bevy_scene::{
    DynamicScene, DynamicSceneBuilder,
    serde::{SceneDeserializer, SceneSerializer},
};
use lentille_render::{
    asset::{AssetRoot, MeshSource, SubAsset},
    camera::RenderTargetConfig,
    picking::resources::Pickable,
    prelude::*,
    shadow::csm::CsmConfig,
    skybox::Environment,
};
use serde::{Deserialize, Serialize, de::DeserializeSeed};
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Component, Clone, Reflect)]
#[reflect(Component)]
pub struct SceneMetadata {
    pub name: String,
    #[entities]
    pub active_camera: Entity,
}
#[derive(Component, Clone, Copy)]
pub struct SceneOwned;

#[derive(Resource, Clone)]
pub struct SceneCodec {
    pub registry: AppTypeRegistry,
    allowed: HashSet<TypeId>,
}
impl Default for SceneCodec {
    fn default() -> Self {
        Self::new()
    }
}
impl SceneCodec {
    pub fn new() -> Self {
        let registry = AppTypeRegistry::default();
        let mut allowed = HashSet::new();
        macro_rules! register {($($ty:ty),+)=> {$(registry.write().register::<$ty>(); allowed.insert(TypeId::of::<$ty>());)+};}
        register!(
            Name,
            SceneMetadata,
            Transform,
            MeshSource,
            PbrMaterial,
            Camera,
            RenderTargetConfig,
            CsmConfig,
            PointLight,
            ParallelLight,
            Environment,
            RotationObject,
            CastShadow,
            MainPassObject,
            Pickable
        );
        for id in &allowed {
            assert!(
                registry
                    .read()
                    .get(*id)
                    .unwrap()
                    .data::<ReflectComponent>()
                    .is_some()
            );
        }
        Self { registry, allowed }
    }
    pub(super) fn persistent_component_ids(
        &self,
        world: &World,
    ) -> Vec<bevy_ecs::component::ComponentId> {
        self.allowed
            .iter()
            .filter_map(|id| world.components().get_id(*id))
            .collect()
    }

    pub fn capture(
        &self,
        world: &World,
        entities: impl Iterator<Item = Entity>,
    ) -> Result<DynamicScene> {
        let registry = world
            .get_resource::<AppTypeRegistry>()
            .context("Missing scene type registry")?;
        for id in &self.allowed {
            ensure!(
                registry
                    .read()
                    .get(*id)
                    .and_then(|r| r.data::<ReflectComponent>())
                    .is_some(),
                "Unregistered persistent component {id:?}"
            );
        }
        let mut filter = bevy_scene::SceneFilter::deny_all();
        for id in &self.allowed {
            filter = filter.allow_by_id(*id);
        }
        Ok(DynamicSceneBuilder::from_world(world)
            .deny_all()
            .with_component_filter(filter)
            .extract_entities(entities)
            .build())
    }
    pub fn encode(&self, scene: &DynamicScene) -> Result<String> {
        #[derive(Serialize)]
        struct Envelope<T> {
            version: u32,
            scene: T,
        }
        let registry = self.registry.read();
        let mut text = ron_scene::ser::to_string_pretty(
            &Envelope {
                version: 1,
                scene: SceneSerializer::new(scene, &registry),
            },
            ron_scene::ser::PrettyConfig::default(),
        )?;
        text.push('\n');
        Ok(text)
    }
    pub fn decode(&self, text: &str) -> Result<DynamicScene> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Envelope {
            version: u32,
            scene: Box<ron_scene::value::RawValue>,
        }
        let envelope: Envelope = ron_scene::from_str(text).context("Scene envelope")?;
        ensure!(
            envelope.version == 1,
            "Unsupported scene version {}",
            envelope.version
        );
        let registry = self.registry.read();
        let mut deserializer = ron_scene::de::Deserializer::from_str(envelope.scene.get_ron())?;
        let scene = SceneDeserializer {
            type_registry: &registry,
        }
        .deserialize(&mut deserializer)
        .context("Scene components")?;
        deserializer.end()?;
        self.check_types(&scene)?;
        Ok(scene)
    }
    fn check_types(&self, scene: &DynamicScene) -> Result<()> {
        ensure!(
            scene.resources.is_empty(),
            "Scene resource snapshots are not allowed"
        );
        let mut entities = HashSet::new();
        for entity in &scene.entities {
            ensure!(entities.insert(entity.entity), "Duplicate scene entity");
            let mut components = HashSet::new();
            for component in &entity.components {
                let id = component
                    .get_represented_type_info()
                    .context("Component without type")?
                    .type_id();
                ensure!(
                    self.allowed.contains(&id),
                    "Component not persistent: {}",
                    component.reflect_type_path()
                );
                ensure!(components.insert(id), "Duplicate scene component");
            }
        }
        for entity in &scene.entities {
            for component in &entity.components {
                if component.represents::<Transform>() {
                    let t = Transform::from_reflect(component.as_partial_reflect())
                        .context("Invalid transform")?;
                    if let Some(parent) = t.parent {
                        ensure!(entities.contains(&parent), "Parent outside scene");
                    }
                }
                if component.represents::<SceneMetadata>() {
                    let m = SceneMetadata::from_reflect(component.as_partial_reflect())
                        .context("Invalid metadata")?;
                    ensure!(
                        entities.contains(&m.active_camera),
                        "Active camera outside scene"
                    );
                }
            }
        }
        Ok(())
    }
    pub fn instantiate(&self, scene: &DynamicScene) -> Result<World> {
        self.check_types(scene)?;
        let mut world = World::new();
        world.insert_resource(self.registry.clone());
        scene.write_to_world_with(&mut world, &mut EntityHashMap::default(), &self.registry)?;
        validate(&mut world)?;
        Ok(world)
    }
    pub fn load(&self, root: &AssetRoot, path: &Path) -> Result<World> {
        ensure!(
            is_scene_file(path),
            "Unsupported scene extension: {}",
            path.display()
        );
        let path = root.resolve_path(path)?;
        ensure!(path.is_file(), "Scene is not a file");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Read scene {}", path.display()))?;
        self.instantiate(&self.decode(&text)?)
    }
}

pub fn is_scene_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.to_ascii_lowercase().ends_with(".scn.ron"))
}
pub fn save_atomic(root: &AssetRoot, path: &Path, text: &str) -> Result<()> {
    ensure!(is_scene_file(path), "Scene file must end with .scn.ron");
    let path = root.save_path(path)?;
    let mut file = tempfile::Builder::new()
        .prefix(".scene-")
        .suffix(".tmp")
        .tempfile_in(path.parent().context("Scene parent")?)?;
    file.write_all(text.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(&path)
        .map_err(|e| e.error)
        .with_context(|| format!("Replace scene {}", path.display()))?;
    Ok(())
}

pub fn validate(world: &mut World) -> Result<()> {
    let roots: Vec<_> = world
        .query::<(Entity, &SceneMetadata)>()
        .iter(world)
        .map(|(e, m)| (e, m.active_camera))
        .collect();
    ensure!(
        roots.len() == 1,
        "Scene must have exactly one metadata root"
    );
    let (root, camera) = roots[0];
    ensure!(
        world.get::<Environment>(root).is_some(),
        "Scene root requires Environment"
    );
    ensure!(
        world.query::<&Camera>().iter(world).count() == 1 && world.get::<Camera>(camera).is_some(),
        "Scene must have one active camera"
    );
    ensure!(
        world.query::<&ParallelLight>().iter(world).count() == 1,
        "Scene requires one directional light"
    );
    ensure!(
        world.query::<&Environment>().iter(world).count() == 1,
        "Scene requires one environment"
    );
    ensure!(
        world.query::<&PointLight>().iter(world).count() <= 128,
        "Point light limit is 128"
    );
    let transforms: HashMap<_, _> = world
        .query::<(Entity, &Transform)>()
        .iter(world)
        .map(|(e, t)| (e, t.clone()))
        .collect();
    let mut parents = HashSet::new();
    for (&e, t) in &transforms {
        ensure!(
            t.position.x.is_finite() && t.position.y.is_finite() && t.position.z.is_finite(),
            "Non-finite position on {e:?}"
        );
        ensure!(
            [t.scale.x, t.scale.y, t.scale.z]
                .iter()
                .all(|v| v.is_finite() && *v >= 0.0001),
            "Invalid scale on {e:?}"
        );
        let q = t.rotation;
        let norm = q.s * q.s + q.v.magnitude2();
        ensure!(
            norm.is_finite() && (norm - 1.).abs() < 0.001,
            "Quaternion must be normalized on {e:?}"
        );
        if let Some(p) = t.parent {
            parents.insert(p);
        }
    }
    lentille_render::transform::compute_world_transforms(&transforms)?;
    for e in parents {
        let t = &transforms[&e];
        ensure!(
            (t.scale.x - t.scale.y).abs() < 0.0001 && (t.scale.x - t.scale.z).abs() < 0.0001,
            "Parent scale must be uniform"
        );
    }
    ensure!(transforms.contains_key(&camera), "Camera transform missing");
    ensure!(
        transforms[&camera].parent.is_none(),
        "Active camera must be a root transform"
    );
    for (e, c) in world.query::<(Entity, &Camera)>().iter(world) {
        ensure!(
            [c.fovy, c.znear, c.zfar].iter().all(|v| v.is_finite())
                && c.fovy > 0.
                && c.fovy < 179.
                && c.znear > 0.
                && c.zfar > c.znear,
            "Invalid camera"
        );
        let csm = world
            .get::<CsmConfig>(e)
            .context("Active camera requires CSM")?;
        ensure!(
            (1..=8).contains(&csm.level_count)
                && csm.texture_size > 0
                && csm.linear_log_factor.is_finite()
                && (0.0..=1.).contains(&csm.linear_log_factor)
                && csm.shadow_near.is_finite()
                && csm.shadow_far.is_finite()
                && csm.shadow_near >= c.znear
                && csm.shadow_far > csm.shadow_near
                && csm.shadow_far <= c.zfar,
            "Invalid CSM settings"
        );
        match world.get::<RenderTargetConfig>(e) {
            Some(RenderTargetConfig::Viewport) => (),
            Some(RenderTargetConfig::Texture { width, height }) => ensure!(
                *width > 0 && *height > 0,
                "Invalid render target dimensions"
            ),
            _ => anyhow::bail!("Saved scenes require Viewport or Texture target"),
        }
    }
    for (e, l) in world.query::<(Entity, &PointLight)>().iter(world) {
        ensure!(transforms.contains_key(&e), "Light transform missing");
        ensure!(
            valid_color(l.color)
                && l.intensity.is_finite()
                && l.intensity >= 0.
                && l.decay.is_finite()
                && l.decay > 0.
                && l.distance.is_none_or(|v| v.is_finite() && v > 0.),
            "Invalid point light"
        );
    }
    for (e, l) in world.query::<(Entity, &ParallelLight)>().iter(world) {
        ensure!(
            transforms.contains_key(&e),
            "Directional light transform missing"
        );
        ensure!(
            valid_color(l.color)
                && [l.intensity, l.size, l.near, l.far]
                    .iter()
                    .all(|v| v.is_finite())
                && l.intensity >= 0.
                && l.size > 0.
                && l.near > 0.
                && l.far > l.near,
            "Invalid directional light"
        );
    }
    for (e, s) in world.query::<(Entity, &MeshSource)>().iter(world) {
        ensure!(transforms.contains_key(&e), "Mesh requires Transform");
        match s {
            MeshSource::Plane { size } => {
                ensure!(size.is_finite() && *size > 0., "Invalid plane size")
            }
            MeshSource::Asset(a) => {
                ensure!(
                    matches!(a.label, SubAsset::Mesh(_)),
                    "Mesh needs mesh selector"
                );
                check_path(&a.path)?;
            }
        }
    }
    for m in world.query::<&PbrMaterial>().iter(world) {
        ensure!(m.color.is_none_or(valid_color), "Invalid material color");
        ensure!(
            [m.roughness, m.metallic, m.reflectance]
                .iter()
                .all(|v| v.is_none_or(|v| v.is_finite() && (0.0..=1.).contains(&v))),
            "Material scalar outside [0,1]"
        );
        use lentille_render::material::pbr::TextureOverride;
        for t in [
            &m.base_color_texture,
            &m.normal_texture,
            &m.metallic_roughness_ao_texture,
            &m.emission_texture,
        ] {
            if let TextureOverride::Set(a) = t {
                check_path(&a.path)?;
                ensure!(
                    matches!(a.label, SubAsset::Image(_) | SubAsset::File),
                    "Invalid texture selector"
                );
            }
        }
    }
    for r in world.query::<&RotationObject>().iter(world) {
        ensure!(r.speed.is_finite(), "Invalid rotation speed");
    }
    if let Some(Environment::Image {
        path,
        size,
        levels,
        samples,
    }) = world.get::<Environment>(root)
    {
        check_path(path)?;
        ensure!(
            *size > 0
                && *levels > 0
                && *levels <= size.ilog2() + 1
                && *samples > 0
                && *samples <= 4096,
            "Invalid environment settings"
        );
    }
    Ok(())
}
fn valid_color(c: Color) -> bool {
    c.into_array().iter().all(|v| v.is_finite() && *v >= 0.)
}
fn check_path(p: &AssetPath) -> Result<PathBuf> {
    let AssetPath::Assets(p) = p;
    AssetRoot::relative(Path::new(p))
}
