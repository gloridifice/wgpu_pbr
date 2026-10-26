use super::{
    codec::{SceneCodec, SceneMetadata},
    controller::{PreparedScene, SceneController, SceneLoadProgress, prepare_camera},
};
use anyhow::{Context, Result, ensure};
use bevy_ecs::prelude::*;
use lentille_render::{
    RenderState,
    asset::{
        AssetRoot, AssetUpload, CpuAssets, FrameBudget, MeshSource, PreparationState,
        SceneAssetCache,
    },
    camera::RenderTargetSize,
    prelude::*,
    shadow::csm::CsmConfig,
    skybox::{
        Environment,
        environment::{CpuEnvironment, EnvironmentUpload},
    },
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Resource, Clone)]
pub struct SceneLoadSettings {
    pub frame_budget: Duration,
    pub upload_bytes_per_frame: usize,
    pub max_cpu_bytes: u64,
    pub max_candidate_gpu_bytes: u64,
    pub max_peak_bytes: u64,
}
impl Default for SceneLoadSettings {
    fn default() -> Self {
        Self {
            frame_budget: Duration::from_millis(2),
            upload_bytes_per_frame: 8 * 1024 * 1024,
            max_cpu_bytes: 1024 * 1024 * 1024,
            max_candidate_gpu_bytes: 1024 * 1024 * 1024,
            max_peak_bytes: 3 * 1024 * 1024 * 1024,
        }
    }
}

struct CpuLoadedScene {
    world: World,
    assets: CpuAssets,
    environment: CpuEnvironment,
    cpu_bytes: u64,
    gpu_bytes: u64,
}
struct Job {
    id: u64,
    path: PathBuf,
    root: AssetRoot,
    codec: SceneCodec,
    settings: SceneLoadSettings,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<String>>,
    old_bytes: u64,
}
impl Job {
    fn checkpoint(&self, message: &str) -> Result<()> {
        ensure!(!self.cancel.load(Ordering::Acquire), "Scene load cancelled");
        *self.progress.lock().unwrap() = message.to_owned();
        Ok(())
    }
    fn load(&self) -> Result<CpuLoadedScene> {
        self.checkpoint("Reading scene")?;
        let mut world = self.codec.load(&self.root, &self.path)?;
        self.checkpoint("Computing transforms")?;
        let transforms: HashMap<_, _> = world
            .query::<(Entity, &Transform)>()
            .iter(&world)
            .map(|(e, t)| (e, t.clone()))
            .collect();
        for (entity, transform) in
            lentille_render::transform::compute_world_transforms(&transforms)?
        {
            world.entity_mut(entity).insert(transform);
        }
        let requests: Vec<_> = world
            .query::<(&MeshSource, Option<&PbrMaterial>)>()
            .iter(&world)
            .map(|(s, m)| (s.clone(), m.cloned()))
            .collect();
        let cpu_limit = self
            .settings
            .max_cpu_bytes
            .min(self.settings.max_peak_bytes.saturating_sub(self.old_bytes));
        let assets = CpuAssets::load(self.root.clone(), &requests, cpu_limit, |stage| {
            self.checkpoint(stage)
        })?;
        self.checkpoint("Preparing environment pixels and SH")?;
        let source = world
            .query::<(&SceneMetadata, &Environment)>()
            .iter(&world)
            .next()
            .context("Missing scene environment")?
            .1;
        let environment = CpuEnvironment::load(
            &self.root,
            source,
            cpu_limit.saturating_sub(assets.cpu_bytes),
        )?;
        self.checkpoint("CPU scene ready")?;
        let cpu_bytes = assets
            .cpu_bytes
            .saturating_add(environment.cpu_bytes)
            .saturating_add(world.entities().len() as u64 * 1024);
        let gpu_bytes = assets.gpu_bytes.saturating_add(environment.gpu_bytes);
        ensure!(cpu_bytes <= cpu_limit, "Scene CPU budget exceeded");
        ensure!(
            gpu_bytes <= self.settings.max_candidate_gpu_bytes
                && self
                    .old_bytes
                    .saturating_add(cpu_bytes)
                    .saturating_add(gpu_bytes)
                    <= self.settings.max_peak_bytes,
            "Scene estimated peak memory budget exceeded"
        );
        Ok(CpuLoadedScene {
            world,
            assets,
            environment,
            cpu_bytes,
            gpu_bytes,
        })
    }
}

struct WorkerResult {
    id: u64,
    result: Result<CpuLoadedScene>,
}
struct Task {
    id: u64,
    path: PathBuf,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<String>>,
    settings: SceneLoadSettings,
    stage: TaskStage,
    started: Instant,
    max_prepare_time: Duration,
    gpu_batches: u64,
}
enum TaskStage {
    Cpu,
    Gpu(Box<GpuScene>),
}
struct GpuScene {
    world: World,
    assets: AssetUpload,
    environment: Option<EnvironmentUpload>,
    environment_data: Option<CpuEnvironment>,
    total_upload_bytes: u64,
    requests: VecDeque<(Entity, MeshSource, Option<PbrMaterial>)>,
    assets_ready: bool,
    environment_ready: bool,
    cpu_bytes: u64,
    gpu_bytes: u64,
    prepared: usize,
}

#[derive(Default)]
struct WorkerInbox {
    slot: Mutex<(Option<Job>, bool)>,
    wake: Condvar,
}

#[derive(Resource)]
pub(super) struct SceneLoader {
    jobs: Arc<WorkerInbox>,
    results: Arc<Mutex<Option<WorkerResult>>>,
    task: Option<Task>,
    next_id: u64,
    in_flight: Option<Arc<AtomicBool>>,
}
impl Default for SceneLoader {
    fn default() -> Self {
        let jobs = Arc::new(WorkerInbox::default());
        let input = jobs.clone();
        let results = Arc::new(Mutex::new(None));
        let output = results.clone();
        std::thread::Builder::new()
            .name("scene-loader".into())
            .spawn(move || {
                lentille_core::profiling::name_thread("scene-loader");
                loop {
                    let job = {
                        let mut slot = input.slot.lock().unwrap();
                        while slot.0.is_none() && !slot.1 {
                            slot = input.wake.wait(slot).unwrap();
                        }
                        let Some(job) = slot.0.take() else {
                            break;
                        };
                        job
                    };
                    let result = {
                        lentille_core::profile_scope!("SceneCpuLoad", job = job.id);
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job.load()))
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("Scene CPU loader panicked")))
                    };
                    let mut slot = output.lock().unwrap();
                    if !job.cancel.load(Ordering::Acquire) {
                        *slot = Some(WorkerResult { id: job.id, result });
                    }
                }
            })
            .expect("Start scene loader worker");
        Self {
            jobs,
            results,
            task: None,
            next_id: 0,
            in_flight: None,
        }
    }
}
impl Drop for SceneLoader {
    fn drop(&mut self) {
        self.cancel();
        let mut slot = self.jobs.slot.lock().unwrap();
        slot.0 = None;
        slot.1 = true;
        self.jobs.wake.notify_one();
    }
}
impl SceneLoader {
    pub fn start(
        &mut self,
        world: &mut World,
        codec: &SceneCodec,
        path: PathBuf,
    ) -> Result<SceneLoadProgress> {
        ensure!(self.task.is_none(), "Scene loader is busy");
        self.next_id = self.next_id.wrapping_add(1);
        let settings = world.resource::<SceneLoadSettings>().clone();
        ensure!(
            settings.upload_bytes_per_frame >= 256 * 1024,
            "Scene upload budget must be at least 256 KiB"
        );
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new("Waiting for CPU worker".to_owned()));
        let job = Job {
            id: self.next_id,
            path: path.clone(),
            root: world.resource::<AssetRoot>().clone(),
            codec: codec.clone(),
            settings: settings.clone(),
            cancel: cancel.clone(),
            progress: progress.clone(),
            old_bytes: active_bytes(world),
        };
        {
            let mut slot = self.jobs.slot.lock().unwrap();
            ensure!(!slot.1, "Scene worker stopped");
            ensure!(
                slot.0
                    .as_ref()
                    .is_none_or(|job| job.cancel.load(Ordering::Acquire)),
                "CPU load already queued"
            );
            slot.0 = Some(job);
            self.jobs.wake.notify_one();
        }
        self.task = Some(Task {
            id: self.next_id,
            path: path.clone(),
            cancel,
            progress,
            settings,
            stage: TaskStage::Cpu,
            started: Instant::now(),
            max_prepare_time: Duration::ZERO,
            gpu_batches: 0,
        });
        Ok(SceneLoadProgress {
            id: self.next_id,
            path,
            stage: "Loading CPU".into(),
            detail: "Waiting for CPU worker".into(),
            uploaded_bytes: 0,
            total_bytes: 0,
        })
    }
    pub fn cancel(&mut self) {
        if let Some(task) = self.task.take() {
            task.cancel.store(true, Ordering::Release);
        }
        self.results.lock().unwrap().take();
    }
    pub fn poll(
        &mut self,
        world: &mut World,
        controller: &mut SceneController,
    ) -> Result<Option<(PreparedScene, PathBuf)>> {
        lentille_core::profile_scope!("SceneUploadPoll");
        if let Some(flag) = &self.in_flight {
            let rs = world.resource::<RenderState>();
            rs.device
                .poll(wgpu::PollType::Poll)
                .context("Poll scene upload")?;
            if flag.load(Ordering::Acquire) {
                self.in_flight = None;
            }
        }
        if let Some(prepared) = self.take_ready(world)? {
            return Ok(Some(prepared));
        }
        let mut completed = None;
        if let Some(result) = self.results.lock().unwrap().take() {
            if self.task.as_ref().is_some_and(|task| task.id == result.id) {
                completed = Some(result.result);
            }
        }
        let Some(mut task) = self.task.take() else {
            return Ok(None);
        };
        let mut touched_gpu = false;
        let result = (|| -> Result<Option<(PreparedScene, PathBuf)>> {
            if let Some(result) = completed {
                let mut loaded = result?;
                let requests = loaded
                    .world
                    .query::<(Entity, &MeshSource, Option<&PbrMaterial>)>()
                    .iter(&loaded.world)
                    .map(|(e, s, m)| (e, s.clone(), m.cloned()))
                    .collect();
                task.stage = TaskStage::Gpu(Box::new(GpuScene {
                    world: loaded.world,
                    total_upload_bytes: loaded
                        .assets
                        .gpu_bytes
                        .saturating_add(loaded.environment.upload_bytes),
                    assets: loaded.assets.into_upload(),
                    environment: None,
                    environment_data: Some(loaded.environment),
                    requests,
                    assets_ready: false,
                    environment_ready: false,
                    cpu_bytes: loaded.cpu_bytes,
                    gpu_bytes: loaded.gpu_bytes,
                    prepared: 0,
                }));
                bevy_log::info!(
                    "Scene CPU ready: {} in {:?}, estimated CPU {} bytes / GPU {} bytes",
                    task.path.display(),
                    task.started.elapsed(),
                    loaded.cpu_bytes,
                    loaded.gpu_bytes
                );
            }
            let Some(progress) = controller.loading.as_mut() else {
                anyhow::bail!("Missing scene loading state");
            };
            progress.detail = task.progress.lock().unwrap().clone();
            let TaskStage::Gpu(gpu) = &mut task.stage else {
                return Ok(None);
            };
            progress.stage = "Preparing GPU".into();
            progress.uploaded_bytes = gpu
                .assets
                .uploaded_bytes
                .saturating_add(gpu.environment.as_ref().map_or(0, |e| e.uploaded_bytes));
            progress.total_bytes = gpu.total_upload_bytes;
            if self.in_flight.is_some() {
                progress.detail = "Waiting for GPU batch".into();
                return Ok(None);
            }
            let total_gpu = gpu
                .gpu_bytes
                .saturating_add(candidate_camera_bytes(&mut gpu.world, world));
            ensure!(
                total_gpu <= task.settings.max_candidate_gpu_bytes
                    && active_bytes(world)
                        .saturating_add(gpu.cpu_bytes)
                        .saturating_add(total_gpu)
                        <= task.settings.max_peak_bytes,
                "Scene estimated peak memory budget exceeded"
            );
            touched_gpu = true;
            let prepare_started = Instant::now();
            let mut budget = FrameBudget::new(
                task.settings.frame_budget,
                task.settings.upload_bytes_per_frame,
            );
            if !gpu.assets_ready {
                gpu.assets_ready = gpu.assets.advance(world, &mut budget)?;
                progress.detail = "Uploading meshes, textures and materials".into();
            }
            if gpu.assets_ready {
                while budget.available() {
                    let Some((e, source, material)) = gpu.requests.pop_front() else {
                        break;
                    };
                    let renderer = gpu
                        .assets
                        .cache
                        .prepare(&source, material.as_ref(), world)
                        .with_context(|| format!("Entity {e:?}, source {source:?}"))?;
                    renderer.update_transform_buffer(
                        &world.resource::<RenderState>().queue,
                        gpu.world.get::<WorldTransform>(e).unwrap().get_uniform(),
                    );
                    gpu.world.entity_mut(e).insert((
                        renderer,
                        PreparationState {
                            source,
                            material,
                            error: None,
                        },
                    ));
                    gpu.prepared += 1;
                    progress.detail = format!("Prepared {} instances", gpu.prepared);
                }
            }
            if gpu.assets_ready
                && gpu.requests.is_empty()
                && !gpu.environment_ready
                && budget.available()
            {
                if let Some(data) = gpu.environment_data.take() {
                    gpu.environment = Some(data.into_upload(world)?);
                }
                gpu.environment_ready = gpu
                    .environment
                    .as_mut()
                    .unwrap()
                    .advance(world, &mut budget)?;
                progress.detail = format!(
                    "Environment step {}",
                    gpu.environment.as_ref().unwrap().completed_steps
                );
            }
            let ready = gpu.assets_ready && gpu.requests.is_empty() && gpu.environment_ready;
            task.max_prepare_time = task.max_prepare_time.max(prepare_started.elapsed());
            task.gpu_batches += 1;
            progress.uploaded_bytes = gpu
                .assets
                .uploaded_bytes
                .saturating_add(gpu.environment.as_ref().map_or(0, |e| e.uploaded_bytes));
            if ready {
                progress.stage = "Ready to commit".into();
            }
            Ok(None)
        })();
        if touched_gpu {
            // Flush writes even after a recoverable preparation error or without an old camera.
            let rs = world.resource::<RenderState>();
            rs.queue.submit([]);
            let done = Arc::new(AtomicBool::new(false));
            let flag = done.clone();
            rs.queue
                .on_submitted_work_done(move || flag.store(true, Ordering::Release));
            self.in_flight = Some(done);
        }
        if result.is_err() {
            task.cancel.store(true, Ordering::Release);
            return result;
        }
        self.task = Some(task);
        result
    }
    pub fn take_ready(&mut self, world: &mut World) -> Result<Option<(PreparedScene, PathBuf)>> {
        if self.in_flight.is_some() {
            return Ok(None);
        }
        let ready = self.task.as_ref().is_some_and(|t| matches!(&t.stage, TaskStage::Gpu(g) if g.assets_ready && g.requests.is_empty() && g.environment_ready));
        if !ready {
            return Ok(None);
        }
        let task = self.task.take().unwrap();
        let TaskStage::Gpu(gpu) = task.stage else {
            unreachable!()
        };
        let GpuScene {
            world: mut candidate,
            assets,
            environment,
            cpu_bytes,
            gpu_bytes,
            ..
        } = *gpu;
        let camera_bytes = candidate_camera_bytes(&mut candidate, world);
        let total_gpu = gpu_bytes.saturating_add(camera_bytes);
        ensure!(
            total_gpu <= task.settings.max_candidate_gpu_bytes
                && active_bytes(world)
                    .saturating_add(cpu_bytes)
                    .saturating_add(total_gpu)
                    <= task.settings.max_peak_bytes,
            "Scene estimated peak memory budget exceeded"
        );
        prepare_camera(world, &mut candidate)?;
        let environment = environment.unwrap();
        let source = environment.source.clone();
        let (skybox, sh) = environment.finish();
        let root = candidate
            .query::<(Entity, &SceneMetadata)>()
            .iter(&candidate)
            .next()
            .unwrap()
            .0;
        candidate.entity_mut(root).insert((
            skybox,
            super::EnvironmentState {
                source,
                error: None,
            },
        ));
        bevy_log::info!(
            "Scene GPU ready: {} after {:?}, estimated candidate GPU {} bytes, {} batches, max CPU prepare {:?}",
            task.path.display(),
            task.started.elapsed(),
            total_gpu,
            task.gpu_batches,
            task.max_prepare_time
        );
        Ok(Some((
            PreparedScene {
                world: candidate,
                cache: assets.finish()?,
                sh,
            },
            task.path,
        )))
    }
}

fn candidate_camera_bytes(candidate: &mut World, world: &mut World) -> u64 {
    let camera = candidate
        .query::<&SceneMetadata>()
        .iter(candidate)
        .next()
        .unwrap()
        .active_camera;
    let config = candidate.get::<CsmConfig>(camera).unwrap();
    let (width, height) = match candidate
        .get::<lentille_render::camera::RenderTargetConfig>(camera)
        .unwrap()
    {
        lentille_render::camera::RenderTargetConfig::Texture { width, height } => (*width, *height),
        _ => world
            .query_filtered::<&RenderTargetSize, With<crate::control::camera::MainCamera>>()
            .iter(world)
            .next()
            .map(|s| (s.width, s.height))
            .unwrap_or((1280, 720)),
    };
    camera_target_bytes(width, height, config)
}

fn camera_target_bytes(width: u32, height: u32, config: &CsmConfig) -> u64 {
    // Preserve the existing per-pixel auxiliary/headroom allowance; base attachments
    // now include two HDR targets and the single shared SDR viewport image.
    lentille_render::camera::RenderTarget::estimated_attachment_bytes(width, height)
        .saturating_add(
            u64::from(width)
                .saturating_mul(u64::from(height))
                .saturating_mul(32),
        )
        .saturating_add(
            u64::from(config.texture_size)
                .pow(2)
                .saturating_mul(u64::from(config.level_count))
                .saturating_mul(4),
        )
}

fn active_bytes(world: &mut World) -> u64 {
    let assets = world
        .get_resource::<SceneAssetCache>()
        .map(|c| c.estimated_bytes())
        .unwrap_or_default();
    let targets = world
        .query::<(&RenderTargetSize, &CsmConfig)>()
        .iter(world)
        .map(|(s, c)| camera_target_bytes(s.width, s.height, c))
        .sum::<u64>();
    let environments = world
        .query::<&Environment>()
        .iter(world)
        .map(|e| match e {
            Environment::Image { size, levels, .. } => (0..*levels)
                .map(|level| u64::from((*size >> level).max(1)).pow(2) * 6 * 8)
                .sum(),
            _ => 0,
        })
        .sum::<u64>();
    assets
        .0
        .saturating_add(assets.1)
        .saturating_add(targets)
        .saturating_add(environments)
}
