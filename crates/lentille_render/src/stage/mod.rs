use crate::{
    FrameSets, SurfaceState,
    camera::{CameraGlobalBindGroup, RenderTarget, TargetType},
    prelude::*,
};
use bevy_app::Plugin;
use bevy_ecs::{prelude::*, system::SystemId};
use wgpu::CommandEncoderDescriptor;

pub(crate) struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_render_system_in_frame_set(
            FrameSets::Draw,
            (sys_render, sys_copy_to_real_target).chain(),
        )
        .add_render_system_in_frame_set(FrameSets::Present, sys_present);
    }
}

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
    pub color_target: Arc<UploadedImage>,
    pub camera_global_bind_group: Arc<BindGroup>,
    depth_target: Option<Arc<UploadedImage>>,
}

pub struct RenderStage {
    pub name: Option<String>,
    pub systems: Vec<SystemId<InMut<'static, RenderContext>>>,
}

pub fn sys_render(
    mut commands: Commands,
    render_stage_manager: Res<RenderStageManager>,
    mut q_camera: Query<(&mut RenderTarget, &mut CameraGlobalBindGroup)>,
    rs: Res<RenderState>,
) {
    for (mut camera_target, mut camera_global_bind_group) in q_camera.iter_mut() {
        for stage in render_stage_manager.stages.iter() {
            let color_target = camera_target.next();
            let depth_target = camera_target.depth.clone();
            let camera_global_bind_group = camera_global_bind_group.next();

            let encoder = rs.device.create_command_encoder(&CommandEncoderDescriptor {
                label: stage.name.as_ref().map(|it| it.as_str()),
            });

            let render_context = RenderContext {
                encoder,
                color_target,
                camera_global_bind_group,
                depth_target,
            };

            let systems = stage.systems.clone();
            commands.queue(move |world: &mut World| {
                let mut ctx = render_context;
                for system in systems {
                    world.run_system_with(system, &mut ctx);
                }
                world
                    .resource::<RenderState>()
                    .queue
                    .submit(std::iter::once(ctx.encoder.finish()));
            });
        }
    }
}

pub fn sys_copy_to_real_target(
    rs: Res<RenderState>,
    q_render_target: Query<&RenderTarget>,
    q_surface_state: Query<&SurfaceState>,
) {
    for target in q_render_target {
        let current_colot = target.get_current_color();
        let size = current_colot.texture.size();
        let texture = match &target.target_type {
            TargetType::WindowAndSurface(entity) => {
                let surface_state = q_surface_state.get(*entity).unwrap();
                &surface_state.surface.get_current_texture().unwrap().texture
            }
            TargetType::Texture(uploaded_image) => &uploaded_image.texture,
        };

        let mut encoder = rs.device.create_command_encoder(&Default::default());
        lentille_wgpu_utils::copy_texture(&mut encoder, &current_colot.texture, texture, size);
        rs.queue.submit(std::iter::once(encoder.finish()));
    }
}

pub fn sys_present(q_surface_state: Query<&SurfaceState>) {
    for surface_state in q_surface_state {
        surface_state
            .surface
            .get_current_texture()
            .unwrap()
            .present();
    }
}
