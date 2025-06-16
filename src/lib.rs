use bevy_app::App;
use bevy_ecs::prelude::*;
use log::info;
use std::sync::Arc;
use wgpu::{Features, Instance, Surface};
use winit::{dpi::PhysicalSize, window::Window};

use crate::window::{ResizeEvent, WindowAndRenderStatePlugin};

mod app;
mod editor;
mod egui_tools;

lazy_static::lazy_static! {
    pub static ref DEVICE_FEATURES: Arc<Vec<Features>> = Arc::new(vec![
        Features::TIMESTAMP_QUERY
    ]);
}

pub fn run() {
    App::new()
        .add_plugins(WindowAndRenderStatePlugin)
        .add_observer(sys_on_resize)
        .run();
}

#[derive(Resource)]
pub enum InsertResourceStage {
    GlobalBindGroupLayot,
}

fn sys_on_resize(event: Trigger<ResizeEvent>, mut rs: ResMut<RenderState>) {
    let new_size = event.physical_size;
    if new_size.width > 0 && new_size.height > 0 {
        rs.size = new_size;
        rs.config.width = new_size.width;
        rs.config.height = new_size.height;
        rs.surface.configure(&rs.device, &rs.config);
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

    let mut move_vec = Vector3::new(0., 0., 0.);
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
            move_vec += Vector3::new(0.0, -1.0, 0.0);
        } else {
            move_vec += Vector3::new(0.0, 1.0, 1.0);
        }
    }
    let delta_time_sec = time.delta_time.as_secs_f32();
    if move_vec != Vector3::new(0., 0., 0.) {
        move_vec = move_vec.normalize() * speed * delta_time_sec;
        cam_transform.position += move_vec;
    }

    let factor = vec2(0.6, 0.4);
    controller.row -= input.cursor_delta.x * factor.x;
    controller.yaw = (controller.yaw - input.cursor_delta.y * factor.y).clamp(-40.0, 80.0);
    cam_transform.rotation = Quaternion::from_angle_y(Deg(controller.row))
        * Quaternion::from_angle_x(Deg(controller.yaw));
}
