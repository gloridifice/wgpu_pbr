use std::any::{TypeId, type_name};

use crate::{
    FrameSets, SurfaceState,
    camera::{CameraGlobalBindGroup, RenderTarget, TargetType},
    graph::{InsertConfig, TypeIdGraph},
    prelude::*,
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
    pub color_target: Arc<UploadedImage>,
    pub camera_global_bind_group: Arc<BindGroup>,
    pub depth_target: Option<Arc<UploadedImage>>,
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
    mut q_camera: Query<(Entity, &mut RenderTarget, &mut CameraGlobalBindGroup)>,
    rs: Res<RenderState>,
) {
    for (camera_id, mut camera_target, mut camera_global_bind_group) in q_camera.iter_mut() {
        render_stage_manager.stages.bfs_mut(|stage| {
            let color_target = camera_target.next();
            let depth_target = camera_target.depth.clone();
            let camera_global_bind_group = camera_global_bind_group.next();

            let encoder = rs.device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some(stage.name),
            });

            let render_context = RenderContext {
                camera_id,
                encoder,
                color_target,
                camera_global_bind_group,
                depth_target,
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
        let texture = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                let surface_state = q_surface_state.get(*entity).unwrap();
                &surface_state.surface.get_current_texture().unwrap().texture
            }
            TargetType::Texture(uploaded_image) => &uploaded_image.texture,
        };

        // The size of current color may be different with surface size
        if size == texture.size() {
            let mut encoder = rs.device.create_command_encoder(&Default::default());
            lentille_wgpu_utils::copy_texture(&mut encoder, &current_color.texture, texture, size);
            rs.queue.submit(std::iter::once(encoder.finish()));
        }
    }
}
