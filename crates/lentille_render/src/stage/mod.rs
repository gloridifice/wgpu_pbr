use std::any::{TypeId, type_name};

use crate::{
    FrameSets, SurfaceState,
    base_assets::{DFGTexture, NoFilterClampSampler, SkyboxSampler},
    bindings::camera_binding::{CameraBindGroupBuilder, CameraBindGroupLayout},
    camera::{CameraBuffer, ColorImage, DepthImage, RenderTarget, TargetType},
    gizmo::{GIZMO_BUFFER, GizmoPrimitive},
    graph::{InsertConfig, TypeIdGraph},
    light::LightUnifromBuffer,
    prelude::*,
    shadow_mapping::csm::CascadeShadowMapping,
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
                (sys_render, sys_copy_to_real_target).chain(),
            );
    }
}

#[derive(Resource, Default)]
pub struct RenderStageManager {
    pub stages: TypeIdGraph<RenderStage>,
}

/// 包含了当前帧渲染上下文的资源，每帧都会重新创建。
/// 其只有在 Prepare 阶段之后存在，并在 Present 阶段删除。
///
/// 所以只能在这两个阶段之间的阶段使用，见 [`crate::RenderSets`].
pub struct RenderContext {
    pub camera_id: Entity,
    pub encoder: wgpu::CommandEncoder,
    pub camera_global_bind_group: Arc<BindGroup>,
    pub color_target: Arc<ColorImage>,
    pub depth_target: Option<Arc<DepthImage>>,
    pub gizmo_primitives: Arc<Vec<GizmoPrimitive>>,
}

type FrameSystemId = SystemId<InMut<'static, RenderContext>>;

/// [`RenderStage`] 管理着一张渲染系统图，允许开发者自定义渲染系统的顺序。
/// 一系列 [`RenderStage`] 由 [`RenderStageManager`] 管理。
pub struct RenderStage {
    pub name: &'static str,
    /// 帧系统图
    pub systems: TypeIdGraph<FrameSystemId>,
}

impl RenderStageManager {
    /// 如果 Stage 未初始化则初始化它，否则什么都不做
    pub fn try_init_stage<Stage: 'static>(&mut self) {
        if self.stages.get::<Stage>().is_none() {
            self.stages
                .add_node(TypeId::of::<Stage>(), Some(RenderStage::new::<Stage>()));
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

impl RenderStage {
    pub fn new<Label: 'static>() -> RenderStage {
        RenderStage {
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
    mut q_camera: Query<(
        Entity,
        &CameraBuffer,
        &mut RenderTarget,
        &CascadeShadowMapping,
    )>,
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

    for (camera_id, camera_buffer, mut camera_target, csm) in q_camera.iter_mut() {
        let color_target = camera_target.next();
        let color_attachment = camera_target.get_attachment_color();
        let depth_target = camera_target.depth.clone();

        render_stage_manager.stages.bfs_mut(|stage| {
            let encoder = rs.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some(stage.name),
            });

            let camera_global_bind_group = {
                let skybox_texture = skybox
                    .single()
                    .ok()
                    .and_then(|it| it.texture.as_ref())
                    .unwrap_or(&default_skybox.texture);

                let device = &rs.device;

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
                        color_target: &color_attachment.view,
                        color_target_sampler: &no_filter_sampler.0,
                        csm_info: &csm.csm_info_buffer,
                    }
                    .build(device, &layout),
                )
            };

            let render_context = RenderContext {
                camera_id,
                encoder,
                color_target: Arc::clone(&color_target),
                camera_global_bind_group,
                depth_target: depth_target.clone(),
                gizmo_primitives: Arc::clone(&gizmo_primitives),
            };

            let systems = stage.systems.clone();
            commands.queue(move |world: &mut World| {
                let mut ctx = render_context;
                for system in systems.into_iter_bfs() {
                    world.run_system_with(system, &mut ctx).unwrap();
                }
                world
                    .resource::<RenderState>()
                    .queue
                    .submit(std::iter::once(ctx.encoder.finish()));
            });
        });
    }
}

pub fn sys_copy_to_real_target(
    rs: Res<RenderState>,
    q_render_target: Query<&RenderTarget>,
    q_surface_state: Query<&SurfaceState>,
) {
    for target in q_render_target {
        let current_color = target.get_current_color();
        let size = current_color.texture.size();
        let mut surface_texture_storage: Option<wgpu::SurfaceTexture> = None;
        let texture: &wgpu::Texture = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                let surface_state = q_surface_state.get(*entity).unwrap();
                match surface_state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(st)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(st) => {
                        surface_texture_storage = Some(st);
                        // Safe: surface_texture_storage outlives this reference via let binding
                        &surface_texture_storage.as_ref().unwrap().texture
                    }
                    status => {
                        bevy_log::warn!("Failed to get current surface texture: {:?}", status);
                        continue;
                    }
                }
            }
            TargetType::Texture(uploaded_image) => uploaded_image.texture.texture(),
        };

        if size == texture.size() {
            let mut encoder = rs.device.create_command_encoder(&Default::default());
            lentille_wgpu_utils::copy_texture2d_to_texture2d_no_mip(
                &mut encoder,
                current_color.texture.texture(),
                texture,
                size,
            );
            rs.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
