use super::{
    codec::{SceneCodec, SceneMetadata, SceneOwned, save_atomic},
    loading::{SceneLoadSettings, SceneLoader},
};
use anyhow::{Result, ensure};
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
    skybox::{ComputedSHUniform, Skybox, SkyboxSHBuffer},
};
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Debug)]
pub enum SceneRequest {
    Open(PathBuf),
    Reload,
    Save,
    SaveAs(PathBuf),
    Retry(Entity),
    CancelLoad,
}
#[derive(Clone, Copy)]
pub enum UnsavedDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct SceneLoadProgress {
    pub id: u64,
    pub path: PathBuf,
    pub stage: String,
    pub detail: String,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
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
    pub loading: Option<SceneLoadProgress>,
    pub(super) saved_baseline: String,
    pub(super) snapshot_cache: super::snapshot_cache::SnapshotCache,
}
impl SceneController {
    fn refresh_dirty(&mut self, world: &mut World, codec: &SceneCodec) -> Result<()> {
        self.dirty = self.snapshot_cache.current(world, codec)? != self.saved_baseline;
        Ok(())
    }

    pub fn busy(&self) -> bool {
        self.request.is_some()
            || self.pending_open.is_some()
            || self.needs_save_path
            || self.loading.is_some()
    }
    pub fn read_only(&self) -> bool {
        self.loading.is_some()
            || self.pending_open.is_some()
            || self.save_then_open
            || self.discard_next_open
            || matches!(
                self.request,
                Some(SceneRequest::Open(_) | SceneRequest::Reload)
            )
    }
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
        if matches!(request, SceneRequest::CancelLoad) && self.loading.is_some() {
            self.request = Some(request);
        } else if !self.busy() {
            self.request = Some(request);
        }
    }
}

pub fn snapshot(world: &mut World, codec: &SceneCodec) -> Result<String> {
    lentille_core::profile_scope!("SceneSnapshotBuild");
    let ids: Vec<_> = world
        .query_filtered::<Entity, With<SceneOwned>>()
        .iter(world)
        .collect();
    codec.encode(&codec.capture(world, ids.into_iter())?)
}
pub fn sys_scene_requests(world: &mut World) {
    if !world.contains_resource::<lentille_render::RenderReady>() {
        return;
    }
    world.init_resource::<SceneLoader>();
    world.init_resource::<SceneLoadSettings>();
    let codec = world.resource::<SceneCodec>().clone();
    world.resource_scope(|world, mut controller: Mut<SceneController>| {
        let request = controller.request.take();
        if controller.loading.is_some() {
            if matches!(request, Some(SceneRequest::CancelLoad)) {
                world.resource_mut::<SceneLoader>().cancel();
                controller.loading = None;
                controller.error = None;
                controller.status = "Scene load cancelled".into();
                return;
            }
            // Recheck here as well: direct request-field writers cannot bypass the gate.
            poll_loading(world, &mut controller, &codec);
            return;
        }
        if controller.path.is_some() {
            if let Err(error) = controller.refresh_dirty(world, &codec) {
                controller.error = Some(format!("{error:#}"));
            }
        }
        // Drain late CPU results and finish polling a cancelled GPU batch while idle.
        if request.is_none() {
            poll_loading(world, &mut controller, &codec);
            return;
        }
        let request = request.unwrap();
        let discard = std::mem::take(&mut controller.discard_next_open);
        let result = (|| -> Result<()> {
            match request {
                SceneRequest::Open(path) => {
                    if controller.dirty && !discard {
                        controller.pending_open = Some(path);
                        return Ok(());
                    }
                    start_loading(world, &mut controller, &codec, path)?;
                }
                SceneRequest::Reload => {
                    if let Some(path) = controller.path.clone() {
                        if controller.dirty {
                            controller.pending_open = Some(path);
                            return Ok(());
                        }
                        start_loading(world, &mut controller, &codec, path)?;
                    }
                }
                SceneRequest::Save | SceneRequest::SaveAs(_) => {
                    ensure!(
                        controller.pending_open.is_none() || controller.save_then_open,
                        "Resolve the pending scene switch first"
                    );
                    let path = match request {
                        SceneRequest::SaveAs(path) => Some(path),
                        _ => controller.path.clone(),
                    };
                    let Some(path) = path else {
                        controller.needs_save_path = true;
                        return Ok(());
                    };
                    let text = snapshot(world, &codec)?;
                    codec.instantiate(&codec.decode(&text)?)?;
                    save_atomic(world.resource::<AssetRoot>(), &path, &text)?;
                    controller.path = Some(path);
                    controller.saved_baseline = text;
                    controller.snapshot_cache = Default::default();
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
                    ensure!(
                        controller.pending_open.is_none(),
                        "Resolve the pending scene switch first"
                    );
                    if world.get::<MeshSource>(e).is_some() {
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
                SceneRequest::CancelLoad => {}
            }
            Ok(())
        })();
        match result {
            Ok(()) => controller.error = None,
            Err(error) => operation_failed(&mut controller, error),
        }
    });
}
fn start_loading(
    world: &mut World,
    controller: &mut SceneController,
    codec: &SceneCodec,
    path: PathBuf,
) -> Result<()> {
    let progress = world
        .resource_scope(|world, mut loader: Mut<SceneLoader>| loader.start(world, codec, path))?;
    controller.status = "Loading scene".into();
    controller.loading = Some(progress);
    Ok(())
}
fn operation_failed(controller: &mut SceneController, error: anyhow::Error) {
    controller.error = Some(format!("{error:#}"));
    controller.status = "Operation failed".into();
    controller.save_then_open = false;
}
fn poll_loading(world: &mut World, controller: &mut SceneController, codec: &SceneCodec) {
    let result =
        world.resource_scope(|world, mut loader: Mut<SceneLoader>| loader.poll(world, controller));
    match result {
        Ok(Some((scene, path))) => {
            let started = Instant::now();
            match commit_scene(world, codec, scene) {
                Ok(baseline) => {
                    controller.path = Some(path);
                    controller.saved_baseline = baseline;
                    controller.snapshot_cache = Default::default();
                    controller.dirty = false;
                    controller.error = None;
                    controller.status = "Scene ready".into();
                    bevy_log::info!("Scene committed in {:?}", started.elapsed());
                }
                Err(error) => operation_failed(controller, error),
            }
            controller.loading = None;
        }
        Ok(None) => {}
        Err(error) => {
            world.resource_mut::<SceneLoader>().cancel();
            controller.loading = None;
            operation_failed(controller, error);
        }
    }
}

pub(super) struct PreparedScene {
    pub world: World,
    pub cache: SceneAssetCache,
    pub sh: ComputedSHUniform,
}
pub(super) fn prepare_camera(world: &mut World, candidate: &mut World) -> Result<()> {
    let active = candidate
        .query::<&SceneMetadata>()
        .iter(candidate)
        .next()
        .unwrap()
        .active_camera;
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
    Ok(())
}

fn commit_scene(world: &mut World, codec: &SceneCodec, scene: PreparedScene) -> Result<String> {
    lentille_core::profile_scope!("SceneCommit");
    let PreparedScene {
        world: mut candidate,
        cache,
        sh,
    } = scene;
    let ids: Vec<_> = candidate.query::<Entity>().iter(&candidate).collect();
    let dynamic = codec.capture(&candidate, ids.iter().copied())?;
    let mut map = EntityHashMap::default();
    let old: Vec<_> = world
        .query_filtered::<Entity, With<SceneOwned>>()
        .iter(world)
        .collect();
    let baseline = (|| -> Result<String> {
        dynamic.write_to_world_with(world, &mut map, &codec.registry)?;
        // Compute the baseline with destination entity IDs before destroying the old scene.
        codec.encode(&codec.capture(world, map.values().copied())?)
    })();
    let baseline = match baseline {
        Ok(value) => value,
        Err(error) => {
            for e in map.values() {
                let _ = world.despawn(*e);
            }
            return Err(error);
        }
    };
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
    Ok(baseline)
}
