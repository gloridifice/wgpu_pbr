use crate::{
    asset::UuidManager,
    camera::{CameraGlobalBindGroup, CameraTarget, RenderTarget},
    prelude::*,
};
use bevy_ecs::{prelude::*, system::SystemId};
use uuid::Uuid;
use wgpu::CommandEncoderDescriptor;

#[derive(Resource)]
pub struct RenderStageManager {
    pub stages: Vec<RenderStage>,
}

/// 包含了当前帧渲染上下文的资源，每帧都会重新创建。
/// 其只有在 Prepare 阶段之后存在，并在 Present 阶段删除。
///
/// 所以只能在这两个阶段之间的阶段使用，见 RenderSets.
pub struct RenderContext {
    pub encoder: wgpu::CommandEncoder,
    pub output_view: wgpu::TextureView,
    pub output_texture: wgpu::SurfaceTexture,
}

#[derive(Debug, Component)]
pub struct RenderContextHandle(pub Uuid);

impl RenderContextHandle {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

type FrameContextManager = UuidManager<RenderContext>;

pub struct RenderStage {
    pub name: Option<String>,
    pub systems: Vec<SystemId<In<RenderContextHandle>>>,
}

pub fn sys_render(
    render_stage_manager: Res<RenderStageManager>,
    q_camera: Query<(&RenderTarget, &CameraGlobalBindGroup)>,
    rs: Res<RenderState>,
) {
    for (camera_target, camera_global_bind_group) in q_camera.iter() {
        for stage in render_stage_manager.stages.iter() {
            let (color_target) = camera_target.current_color;

            let encoder = rs.device.create_command_encoder(&CommandEncoderDescriptor {
                label: stage.name.as_ref(),
            });

            let render_context = RenderContext {
                encoder,
                output_view: todo!(),
                output_texture: todo!(),
            };

            let 
        }
    }
}
