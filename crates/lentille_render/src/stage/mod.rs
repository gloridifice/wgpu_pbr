use std::{
    any::{TypeId, type_name},
    marker::PhantomData,
};

use crate::{
    FrameSets, SurfaceState,
    base_assets::{DFGTexture, NoFilterClampSampler, SkyboxSampler},
    bindings::camera_binding::{CameraBindGroupBuilder, CameraBindGroupLayout},
    blit::{BlitPipeline, BlitShader, WindowBlitPipelines},
    camera::{CameraBuffer, ColorImage, DepthImage, HdrColorTargets, RenderTarget, TargetType},
    gizmo::{GIZMO_BUFFER, GizmoPrimitive},
    graph::{InsertConfig, TypeIdGraph},
    light::LightUnifromBuffer,
    prelude::*,
    shadow::csm::CascadeShadowMapping,
    skybox::{DefaultSkybox, SkyboxSHBuffer},
};
use bevy_app::Plugin;
use bevy_ecs::{prelude::*, system::SystemId};
use wgpu::CommandEncoderDescriptor;

pub(crate) struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<RenderStageManager>()
            .add_render_system_in_frame_set(
                FrameSets::Draw,
                (sys_render, sys_present_render_targets).chain(),
            );
    }
}

#[derive(Resource, Default)]
pub struct RenderStageManager {
    pub stages: TypeIdGraph<RenderStageInstance>,
}

pub struct RenderStageConfig<T> {
    pub configs: Vec<InsertConfig>,
    pub _p: PhantomData<T>,
}

#[allow(unused)]
impl<T> RenderStageConfig<T> {
    fn after<R: 'static>(mut self, _a: R) -> Self {
        self.configs.push(InsertConfig::After(TypeId::of::<R>()));
        self
    }
    fn before<R: 'static>(mut self, _a: R) -> Self {
        self.configs.push(InsertConfig::Before(TypeId::of::<R>()));
        self
    }
}

pub trait RenderStage {
    type S: 'static;
    fn after<T: 'static>(self, _a: T) -> RenderStageConfig<Self::S>;
    fn before<T: 'static>(self, _b: T) -> RenderStageConfig<Self::S>;
}

impl<T: 'static + RenderStage> From<T> for RenderStageConfig<T> {
    fn from(_value: T) -> Self {
        RenderStageConfig {
            configs: vec![],
            _p: PhantomData,
        }
    }
}

/// One camera/stage invocation. HDR state is carried to the next stage;
/// display-space passes explicitly use `display_target` after tone mapping.
pub struct RenderContext {
    pub camera_id: Entity,
    pub encoder: wgpu::CommandEncoder,
    pub camera_global_bind_group: Arc<BindGroup>,
    pub hdr: HdrColorTargets,
    pub display_target: Arc<ColorImage>,
    pub depth_target: Option<Arc<DepthImage>>,
    pub gizmo_primitives: Arc<Vec<GizmoPrimitive>>,
}

type FrameSystemId = SystemId<InMut<'static, RenderContext>>;

/// [`RenderStage`] 管理着一张渲染系统图，允许开发者自定义渲染系统的顺序。
/// 一系列 [`RenderStage`] 由 [`RenderStageManager`] 管理。
pub struct RenderStageInstance {
    pub name: &'static str,
    /// 帧系统图
    pub systems: TypeIdGraph<FrameSystemId>,
}

impl RenderStageManager {
    /// 如果 Stage 未初始化则初始化它，否则什么都不做
    pub fn try_init_stage<Stage: 'static>(&mut self) {
        if self.stages.get::<Stage>().is_none() {
            self.stages.add_node(
                TypeId::of::<Stage>(),
                Some(RenderStageInstance::new::<Stage>()),
            );
        }
    }

    pub fn insert_system<Stage: 'static, L: 'static>(
        &mut self,
        id: FrameSystemId,
        configs: impl Into<Vec<InsertConfig>>,
    ) {
        self.try_init_stage::<Stage>();
        if let Some(render_stage) = self.stages.get_mut::<Stage>() {
            render_stage.insert_with_configs::<L>(id, configs);
        }
    }
}

impl RenderStageInstance {
    pub fn new<Label: 'static>() -> RenderStageInstance {
        RenderStageInstance {
            name: type_name::<Label>(),
            systems: Default::default(),
        }
    }

    /// L: Label 标签，该渲染系统的唯一标识符
    pub fn insert_with_configs<L: 'static>(
        &mut self,
        system_id: FrameSystemId,
        configs: impl Into<Vec<InsertConfig>>,
    ) {
        self.systems
            .insert_with_configs::<L>(system_id, configs.into());
    }
}

pub fn sys_render(
    mut commands: Commands,
    mut render_stage_manager: ResMut<RenderStageManager>,
    q_camera: Query<(Entity, &CameraBuffer, &RenderTarget, &CascadeShadowMapping)>,
    rs: Res<RenderState>,

    light: Res<LightUnifromBuffer>,
    dfg: Res<DFGTexture>,
    layout: Res<CameraBindGroupLayout>,
    no_filter_sampler: Res<NoFilterClampSampler>,
    skybox_sampler: Res<SkyboxSampler>,

    default_skybox: Res<DefaultSkybox>,
    skybox_sh: Res<SkyboxSHBuffer>,
    skybox: Query<&Skybox>,
) {
    let gizmo_primitives = {
        let mut buf = GIZMO_BUFFER.lock().unwrap();
        if buf.is_empty() {
            Arc::new(Vec::new())
        } else {
            Arc::new(buf.drain(..).collect())
        }
    };

    let mut stages = Vec::new();
    render_stage_manager.stages.bfs_mut(|stage| {
        stages.push((stage.name, stage.systems.clone()));
    });

    for (camera_id, camera_buffer, camera_target, csm) in &q_camera {
        if !csm.initialized {
            continue;
        }
        let mut hdr = camera_target.hdr.clone();
        let display_target = camera_target.display_color();
        let depth_target = camera_target.depth.clone();
        let skybox_texture = skybox
            .single()
            .ok()
            .and_then(|it| it.texture.as_ref())
            .unwrap_or(&default_skybox.texture);
        let camera_bind_groups = hdr.images.each_ref().map(|image| {
            Arc::new(
                CameraBindGroupBuilder {
                    camera_uniform: &camera_buffer.buffer,
                    light_uniform: &light.buffer,
                    csm_texture: &csm.full_view,
                    csm_sampler: &csm.sampler,
                    dfg: &dfg.texture.view,
                    skybox_texture: &skybox_texture.view,
                    skybox_sampler: &skybox_sampler.0,
                    skybox_sh_uniform: &skybox_sh.buffer,
                    color_target: &image.view,
                    color_target_sampler: &no_filter_sampler.0,
                    csm_info: &csm.csm_info_buffer,
                }
                .build(&rs.device, &layout),
            )
        });
        let stages = stages.clone();
        let gizmo_primitives = Arc::clone(&gizmo_primitives);

        // Construct each context only after the previous stage has executed.
        // Keep per-stage submission, but never lose a pass's ping-pong swap.
        commands.queue(move |world: &mut World| {
            for (name, systems) in stages {
                let encoder = world
                    .resource::<RenderState>()
                    .device
                    .create_command_encoder(&CommandEncoderDescriptor { label: Some(name) });
                let mut ctx = RenderContext {
                    camera_id,
                    encoder,
                    camera_global_bind_group: Arc::clone(
                        &camera_bind_groups[hdr.alternate_index()],
                    ),
                    hdr,
                    display_target: Arc::clone(&display_target),
                    depth_target: depth_target.clone(),
                    gizmo_primitives: Arc::clone(&gizmo_primitives),
                };
                for system in systems.into_iter_bfs() {
                    world.run_system_with(system, &mut ctx).unwrap();
                }
                world
                    .resource::<RenderState>()
                    .queue
                    .submit([ctx.encoder.finish()]);
                hdr = ctx.hdr;
            }
        });
    }
}

pub fn sys_present_render_targets(
    rs: Res<RenderState>,
    q_render_target: Query<&RenderTarget>,
    q_surface_state: Query<&SurfaceState>,
    shader: Res<BlitShader>,
    sampler: Res<SkyboxSampler>,
    mut pipelines: ResMut<WindowBlitPipelines>,
) {
    for target in q_render_target {
        // Texture targets already share the SDR display image; no copy required.
        let TargetType::WindowAndSurface(entity) = &target.target_type else {
            continue;
        };
        let Ok(surface) = q_surface_state.get(*entity) else {
            continue;
        };
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            status => {
                bevy_log::warn!("Failed to get current surface texture: {status:?}");
                continue;
            }
        };
        let pipeline = pipelines
            .0
            .entry(surface.config.format)
            .or_insert_with(|| BlitPipeline::new(&rs.device, &shader.0, surface.config.format));
        crate::blit::blit(
            &rs.device,
            &rs.queue,
            pipeline,
            &sampler.0,
            &target.display_color().view,
            &frame.texture.create_view(&Default::default()),
        );
        frame.present();
    }
}
