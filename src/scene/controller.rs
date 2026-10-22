use super::codec::{SceneCodec, SceneMetadata, SceneOwned, save_atomic, validate};
use anyhow::{Context, Result, ensure};
use bevy_ecs::{entity::EntityHashMap, prelude::*};
use lentille_render::{
    asset::{AssetRoot, MeshSource, PreparationState, SceneAssetCache},
    camera::{
        CameraBuffer, CameraState, RenderTarget, RenderTargetConfig, RenderTargetResizedEvent,
        RenderTargetSize,
    },
    deferred_rendering::write_g_buffer_pipeline::{
        GBufferTextureBindGroup, GBufferTextureBindGroupLayout,
    },
    picking::resources::{LastPickResult, PickRequest, PickingIDMap, SelectedObjects},
    prelude::*,
    shadow::csm::{CascadeShadowMapping, CsmConfig, CsmShader},
    skybox::{ComputedSHUniform, Environment, Skybox, SkyboxSHBuffer, prepare_environment},
};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone, Debug)]
pub enum SceneRequest {
    Open(PathBuf),
    Reload,
    Save,
    SaveAs(PathBuf),
    Retry(Entity),
}
#[derive(Clone, Copy)]
pub enum UnsavedDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Resource, Default)]
pub struct SceneController {
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub error: Option<String>,
    pub status: String,
    pub request: Option<SceneRequest>,
    pub pending_open: Option<PathBuf>,
    pub needs_save_path: bool,
    pub save_then_open: bool,
    pub discard_next_open: bool,
    pub refresh: u64,
    pub(super) saved_baseline: String,
}
impl SceneController {
    pub fn resolve_unsaved(&mut self, decision: UnsavedDecision) {
        if self.pending_open.is_none() {
            return;
        }
        match decision {
            UnsavedDecision::Save => {
                self.save_then_open = true;
                self.request = Some(SceneRequest::Save);
            }
            UnsavedDecision::Discard => {
                self.request = self.pending_open.take().map(SceneRequest::Open);
                self.discard_next_open = true;
            }
            UnsavedDecision::Cancel => {
                self.pending_open = None;
                self.save_then_open = false;
            }
        }
    }
    pub fn request(&mut self, request: SceneRequest) {
        if self.request.is_none() && self.pending_open.is_none() {
            self.request = Some(request);
        }
    }
}

pub fn snapshot(world: &mut World, codec: &SceneCodec) -> Result<String> {
    let ids: Vec<_> = world
        .query_filtered::<Entity, With<SceneOwned>>()
        .iter(world)
        .collect();
    let scene = codec.capture(world, ids.into_iter())?;
    codec.encode(&scene)
}
pub fn sys_scene_requests(world: &mut World) {
    if !world.contains_resource::<lentille_render::RenderReady>() {
        return;
    }
    let codec = world.resource::<SceneCodec>().clone();
    world.resource_scope(|world, mut controller: Mut<SceneController>| {
        if controller.path.is_some() {
            match snapshot(world, &codec) {
                Ok(value) => controller.dirty = value != controller.saved_baseline,
                Err(error) => controller.error = Some(format!("{error:#}")),
            }
        }
        let Some(request) = controller.request.take() else {
            return;
        };
        let discard = std::mem::take(&mut controller.discard_next_open);
        let result = (|| -> Result<()> {
            match request {
                SceneRequest::Open(path) => {
                    if controller.dirty && !discard {
                        controller.pending_open = Some(path);
                        return Ok(());
                    }
                    open_scene(world, &codec, &path)?;
                    controller.path = Some(path);
                    controller.saved_baseline = snapshot(world, &codec)?;
                    controller.dirty = false;
                    controller.status = "Scene ready".into();
                }
                SceneRequest::Reload => {
                    if let Some(path) = controller.path.clone() {
                        if controller.dirty {
                            controller.pending_open = Some(path);
                            return Ok(());
                        }
                        open_scene(world, &codec, &path)?;
                        controller.saved_baseline = snapshot(world, &codec)?;
                        controller.dirty = false;
                        controller.status = "Scene reloaded".into();
                    }
                }
                SceneRequest::Save | SceneRequest::SaveAs(_) => {
                    let path = match request {
                        SceneRequest::SaveAs(path) => Some(path),
                        _ => controller.path.clone(),
                    };
                    let Some(path) = path else {
                        controller.needs_save_path = true;
                        return Ok(());
                    };
                    let text = snapshot(world, &codec)?;
                    // Validate structure without requiring successfully prepared assets.
                    codec.instantiate(&codec.decode(&text)?)?;
                    save_atomic(world.resource::<AssetRoot>(), &path, &text)?;
                    controller.path = Some(path);
                    controller.saved_baseline = text;
                    controller.dirty = false;
                    controller.refresh += 1;
                    controller.status = "Scene saved".into();
                    controller.needs_save_path = false;
                    if controller.save_then_open {
                        controller.save_then_open = false;
                        if let Some(next) = controller.pending_open.take() {
                            controller.request = Some(SceneRequest::Open(next));
                        }
                    }
                }
                SceneRequest::Retry(e) => {
                    if world.get::<MeshSource>(e).is_some() {
                        // Explicit retry also rereads repaired source files. Rebuild
                        // this small scene's scope rather than retaining stale imports.
                        let root = world.resource::<AssetRoot>().clone();
                        let entities: Vec<_> = world
                            .query_filtered::<Entity, With<MeshSource>>()
                            .iter(world)
                            .collect();
                        for entity in entities {
                            world
                                .entity_mut(entity)
                                .remove::<(MeshRenderer, PreparationState)>();
                        }
                        world.insert_resource(SceneAssetCache::new(root));
                    }
                    if let Ok(mut entity) = world.get_entity_mut(e) {
                        entity.remove::<super::EnvironmentState>();
                    }
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => controller.error = None,
            Err(error) => {
                controller.error = Some(format!("{error:#}"));
                controller.status = "Operation failed".into();
                controller.save_then_open = false;
            }
        }
    });
}

struct PreparedScene {
    world: World,
    cache: SceneAssetCache,
    sh: ComputedSHUniform,
}
fn prepare_scene(world: &mut World, mut candidate: World) -> Result<PreparedScene> {
    validate(&mut candidate)?;
    let mut cache = SceneAssetCache::new(world.resource::<AssetRoot>().clone());
    let transforms: HashMap<_, _> = candidate
        .query::<(Entity, &Transform)>()
        .iter(&candidate)
        .map(|(e, t)| (e, t.clone()))
        .collect();
    for (entity, transform) in lentille_render::transform::compute_world_transforms(&transforms)? {
        candidate.entity_mut(entity).insert(transform);
    }
    let requests: Vec<_> = candidate
        .query::<(Entity, &MeshSource, Option<&PbrMaterial>)>()
        .iter(&candidate)
        .map(|(e, s, m)| (e, s.clone(), m.cloned()))
        .collect();
    for (e, source, material) in requests {
        let renderer = cache
            .prepare(&source, material.as_ref(), world)
            .with_context(|| format!("Entity {e:?}, source {source:?}"))?;
        renderer.update_transform_buffer(
            &world.resource::<RenderState>().queue,
            candidate.get::<WorldTransform>(e).unwrap().get_uniform(),
        );
        candidate.entity_mut(e).insert((
            renderer,
            PreparationState {
                source,
                material,
                error: None,
            },
        ));
    }
    let (root, active) = candidate
        .query::<(Entity, &SceneMetadata)>()
        .iter(&candidate)
        .next()
        .map(|(e, m)| (e, m.active_camera))
        .unwrap();
    let env = candidate.get::<Environment>(root).unwrap().clone();
    let (skybox, sh) = prepare_environment(world, &env)?;
    candidate.entity_mut(root).insert((
        skybox,
        crate::scene::EnvironmentState {
            source: env,
            error: None,
        },
    ));
    let camera = candidate.get::<Camera>(active).unwrap().clone();
    let config = candidate.get::<CsmConfig>(active).unwrap().clone();
    let size = match candidate.get::<RenderTargetConfig>(active).unwrap() {
        RenderTargetConfig::Texture { width, height } => (*width, *height),
        _ => world
            .query_filtered::<&RenderTargetSize, With<crate::control::camera::MainCamera>>()
            .iter(world)
            .next()
            .map(|s| (s.width.max(1), s.height.max(1)))
            .unwrap_or((1280, 720)),
    };
    let rs = world.resource::<RenderState>();
    ensure!(
        size.0 <= rs.device.limits().max_texture_dimension_2d
            && size.1 <= rs.device.limits().max_texture_dimension_2d
            && config.texture_size <= rs.device.limits().max_texture_dimension_2d,
        "Scene texture size exceeds device limits"
    );
    let target = RenderTarget::new_texture_target(
        size.0,
        size.1,
        lentille_render::SCREEN_FORMAT,
        &rs.device,
    );
    let buffer = CameraBuffer::new(&rs.device);
    let transform = candidate.get::<WorldTransform>(active).unwrap();
    buffer.buffer.write(
        camera.get_uniform(transform, [size.0 as f32, size.1 as f32]),
        &rs.queue,
    );
    let state = CameraState {
        aspect: size.0 as f32 / size.1 as f32,
        view_proj: camera.build_view_projection_matrix(transform, size.0 as f32 / size.1 as f32),
    };
    let csm = CascadeShadowMapping::new(
        &config,
        rs,
        world.resource::<CsmShader>().0.clone(),
        world.resource::<ObjectBindGroupLayout>(),
    );
    let gbuffer = GBufferTextureBindGroup::new(
        &rs.device,
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        world.resource::<GBufferTextureBindGroupLayout>(),
        &target.depth.as_ref().unwrap().view,
    );
    candidate.entity_mut(active).insert((
        target,
        buffer,
        state,
        csm,
        gbuffer,
        RenderTargetSize {
            width: size.0,
            height: size.1,
        },
    ));
    Ok(PreparedScene {
        world: candidate,
        cache,
        sh,
    })
}

pub fn open_scene(world: &mut World, codec: &SceneCodec, path: &std::path::Path) -> Result<()> {
    let candidate = codec.load(world.resource::<AssetRoot>(), path)?;
    let PreparedScene {
        world: mut candidate,
        cache,
        sh,
    } = prepare_scene(world, candidate)?;
    let ids: Vec<_> = candidate.query::<Entity>().iter(&candidate).collect();
    let dynamic = codec.capture(&candidate, ids.iter().copied())?;
    let mut map = EntityHashMap::default();
    let old: Vec<_> = world
        .query_filtered::<Entity, With<SceneOwned>>()
        .iter(world)
        .collect();
    if let Err(error) = dynamic.write_to_world_with(world, &mut map, &codec.registry) {
        for e in map.values() {
            let _ = world.despawn(*e);
        }
        return Err(error.into());
    }
    // No schedules run while both generations exist. All recoverable work is complete.
    for (source, target) in &map {
        let mut from = candidate.entity_mut(*source);
        macro_rules! transfer {($($ty:ty),+)=> {$(if let Some(value)=from.take::<$ty>() {world.entity_mut(*target).insert(value);})+};}
        transfer!(
            MeshRenderer,
            PreparationState,
            WorldTransform,
            RenderTarget,
            RenderTargetSize,
            CameraBuffer,
            CameraState,
            CascadeShadowMapping,
            GBufferTextureBindGroup,
            Skybox,
            super::EnvironmentState
        );
        world.entity_mut(*target).insert(SceneOwned);
    }
    for e in old {
        let _ = world.despawn(e);
    }
    world.insert_resource(cache);
    world
        .resource::<SkyboxSHBuffer>()
        .buffer
        .write(sh, &world.resource::<RenderState>().queue);
    if let Some(mut selected) = world.get_resource_mut::<SelectedObjects>() {
        selected.clear();
    }
    if let Some(mut result) = world.get_resource_mut::<LastPickResult>() {
        *result = LastPickResult::default();
    }
    world.remove_resource::<PickRequest>();
    if let Some(mut epoch) =
        world.get_resource_mut::<lentille_render::picking::resources::PickSceneEpoch>()
    {
        epoch.current = epoch.current.wrapping_add(1);
    }
    if let Some(mut ids) = world.get_resource_mut::<PickingIDMap>() {
        ids.entity_to_id.clear();
        ids.id_to_entity.clear();
    }
    crate::editor::clear_scene_ui(world);
    let camera = world
        .query::<&SceneMetadata>()
        .iter(world)
        .next()
        .unwrap()
        .active_camera;
    let transform = world.get::<Transform>(camera).unwrap();
    let euler = Euler::from(transform.rotation);
    let controller = crate::control::camera::CameraController {
        row: Deg::from(euler.y).0,
        yaw: Deg::from(euler.x).0,
    };
    world
        .entity_mut(camera)
        .insert((crate::control::camera::MainCamera, controller));
    let size = world.get::<RenderTargetSize>(camera).unwrap();
    let event = RenderTargetResizedEvent {
        render_target_entity: camera,
        new_width: size.width,
        new_height: size.height,
    };
    world.trigger(event);
    Ok(())
}
