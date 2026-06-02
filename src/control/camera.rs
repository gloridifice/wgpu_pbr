use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_log::info;
use lentille_core::{input::Input, time::Time};
use lentille_math::*;
use lentille_render::{
    camera::Camera,
    prelude::{Transform, WorldTransform},
};
use winit::keyboard::KeyCode;

use crate::control::ControlState;

pub struct CameraControlPlugin;

impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<CameraConfig>()
            .add_systems(Update, sys_update_camera_control);
    }
}

#[derive(Component, Clone, Default)]
pub struct CameraController {
    pub row: f32,
    pub yaw: f32,
}

#[derive(Resource)]
pub struct CameraConfig {
    pub speed: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self { speed: 5.0 }
    }
}

pub fn sys_update_camera_control(
    config: Res<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    control_state: Res<ControlState>,
    camera_query: Single<(
        &Camera,
        &mut Transform,
        &WorldTransform,
        &mut CameraController,
    )>,
) {
    if !control_state.is_focused {
        return;
    }

    let (_, mut cam_transform, world_trans, mut controller) = camera_query.into_inner();

    let speed = config.speed;

    let mut move_vec = Vec3::new(0., 0., 0.);
    if input.is_key_hold(KeyCode::KeyW) {
        move_vec += world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyA) {
        move_vec += world_trans.left();
    }
    if input.is_key_hold(KeyCode::KeyS) {
        move_vec -= world_trans.forward();
    }
    if input.is_key_hold(KeyCode::KeyD) {
        move_vec -= world_trans.left();
    }
    if input.is_key_hold(KeyCode::Space) {
        if input.is_key_hold(KeyCode::ShiftLeft) {
            move_vec += Vec3::new(0.0, -1.0, 0.0);
        } else {
            move_vec += Vec3::new(0.0, 1.0, 0.0);
        }
    }
    let delta_time_sec = time.delta_time.as_secs_f32();
    if move_vec != Vec3::new(0., 0., 0.) {
        move_vec = move_vec.normalize() * speed * delta_time_sec;
        cam_transform.position += move_vec;
    }

    let factor = Vec2::new(0.6, 0.4);
    controller.row -= input.cursor_delta.x * factor.x;
    controller.yaw = (controller.yaw - input.cursor_delta.y * factor.y).clamp(-40.0, 80.0);
    cam_transform.rotation =
        Quat::from_angle_y(Deg(controller.row)) * Quat::from_angle_x(Deg(controller.yaw));
}
