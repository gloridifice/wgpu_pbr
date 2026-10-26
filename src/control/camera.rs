use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use lentille_core::{input::Input, time::Time};
use lentille_math::*;
use lentille_render::{
    asset::SceneAssetCache,
    camera::{Camera, CameraState},
    picking::resources::SelectedObjects,
    prelude::{MeshRenderer, Transform, WorldTransform},
};
use winit::keyboard::KeyCode;

use super::{ControlState, NavigationMode};

#[cfg(test)]
mod tests;

pub struct CameraControlPlugin;
impl Plugin for CameraControlPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.init_resource::<CameraConfig>()
            .init_resource::<OrbitFocus>()
            .add_systems(
                Update,
                sys_update_camera_control
                    .run_if(crate::scene::scene_editable)
                    .after(crate::editor::EditorUiSet)
                    .after(super::sys_input_implement),
            );
    }
}

#[derive(Component, Default, Clone, Copy)]
pub struct MainCamera;

#[derive(Component, Clone, Default, PartialEq)]
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
        Self { speed: 10.0 }
    }
}

#[derive(Resource, Default)]
struct OrbitFocus {
    camera: Option<Entity>,
    pivot: Option<Vec3>,
}

type Objects<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static WorldTransform,
        Option<&'static MeshRenderer>,
        &'static Transform,
    ),
    Without<MainCamera>,
>;

// Include descendants of selected hierarchy nodes, using retained CPU geometry.
fn selection_bounds(
    selected: &SelectedObjects,
    objects: &Objects,
    assets: &SceneAssetCache,
) -> Option<(Vec3, f32)> {
    let mut min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
    let mut max = -min;
    let mut include = |p: Vec3| {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    };
    for (entity, world, renderer, local) in objects.iter() {
        let mut chosen = selected.entities.contains(&entity);
        let mut parent = local.parent;
        for _ in 0..objects.iter().len() {
            if chosen {
                break;
            }
            let Some(id) = parent else {
                break;
            };
            chosen = selected.entities.contains(&id);
            parent = objects.get(id).ok().and_then(|(_, _, _, t)| t.parent);
        }
        if !chosen {
            continue;
        }
        if let Some(mesh) = renderer
            .and_then(|r| assets.mesh_data(r.mesh))
            .filter(|m| !m.vertices.is_empty())
        {
            for vertex in &mesh.vertices {
                let p = Vec3::from(vertex.position);
                include(
                    world.position
                        + world.rotation
                            * Vec3::new(
                                p.x * world.scale.x,
                                p.y * world.scale.y,
                                p.z * world.scale.z,
                            ),
                );
            }
        } else {
            // Lights and empty nodes have a small, usable framing extent.
            include(world.position - Vec3::new(0.5, 0.5, 0.5));
            include(world.position + Vec3::new(0.5, 0.5, 0.5));
        }
    }
    min.x
        .is_finite()
        .then(|| ((min + max) * 0.5, ((max - min).magnitude() * 0.5).max(0.1)))
}

fn sys_update_camera_control(
    mut config: ResMut<CameraConfig>,
    input: Res<Input>,
    time: Res<Time>,
    control: Res<ControlState>,
    selected: Res<SelectedObjects>,
    assets: Res<SceneAssetCache>,
    objects: Objects,
    mut focus: ResMut<OrbitFocus>,
    mut cameras: Query<
        (
            Entity,
            &Camera,
            &CameraState,
            &mut Transform,
            &WorldTransform,
            &mut CameraController,
        ),
        With<MainCamera>,
    >,
) {
    if !input.window_focused {
        return;
    }
    let Ok((entity, camera, camera_state, mut transform, world, mut controller)) =
        cameras.single_mut()
    else {
        return;
    };
    if focus.camera != Some(entity) {
        focus.camera = Some(entity);
        focus.pivot = None;
    }
    let focusing = control.viewport_active && input.is_key_down(KeyCode::KeyF);
    if control.mode == NavigationMode::Idle && !focusing {
        return;
    }
    let mut position = world.position;
    let mut rotation = world.rotation;
    let forward = rotation * Vec3::new(0.0, 0.0, -1.0);
    if focusing {
        if let Some((center, radius)) = selection_bounds(&selected, &objects, &assets) {
            let half_vertical = camera.fovy.to_radians() * 0.5;
            let half_horizontal = (half_vertical.tan() * camera_state.aspect.max(0.01)).atan();
            let distance = (radius * 1.1 / half_vertical.min(half_horizontal).sin())
                .max(radius + camera.znear * 2.0);
            position = center - forward * distance;
            focus.pivot = Some(center);
        }
    }
    if control.just_started {
        // Derive angles from the current world direction (also after property edits).
        controller.row = (-forward.x).atan2(-forward.z).to_degrees();
        controller.yaw = forward
            .y
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees()
            .clamp(-89.0, 89.0);
        if control.mode == NavigationMode::Orbit {
            let center = selection_bounds(&selected, &objects, &assets)
                .map(|(c, _)| c)
                .or(focus.pivot)
                .unwrap_or(position + forward * 10.0);
            focus.pivot = Some(center);
            let direction = center - position;
            if direction.magnitude2() > 0.0001 {
                let direction = direction.normalize();
                controller.row = (-direction.x).atan2(-direction.z).to_degrees();
                controller.yaw = direction
                    .y
                    .clamp(-1.0, 1.0)
                    .asin()
                    .to_degrees()
                    .clamp(-89.0, 89.0);
            }
        }
    }
    if control.mode != NavigationMode::Idle {
        if !control.just_started {
            controller.row -= input.cursor_delta.x * 0.2;
            controller.yaw = (controller.yaw - input.cursor_delta.y * 0.2).clamp(-89.0, 89.0);
        }
        rotation =
            Quat::from_angle_y(Deg(controller.row)) * Quat::from_angle_x(Deg(controller.yaw));
        let forward = rotation * Vec3::new(0.0, 0.0, -1.0);
        if control.mode == NavigationMode::Orbit {
            let pivot = focus.pivot.unwrap();
            position = pivot - forward * (position - pivot).magnitude().max(0.1);
        } else {
            config.speed = (config.speed * 1.2_f32.powf(input.wheel_delta)).clamp(0.1, 10000.0);
            let axis = |positive, negative| {
                (input.is_key_hold(positive) as i32 - input.is_key_hold(negative) as i32) as f32
            };
            let movement = forward * axis(KeyCode::KeyW, KeyCode::KeyS)
                + (rotation * Vec3::unit_x()) * axis(KeyCode::KeyD, KeyCode::KeyA)
                + Vec3::unit_y() * axis(KeyCode::KeyE, KeyCode::KeyQ);
            let boost = if input.is_key_hold(KeyCode::ShiftLeft)
                || input.is_key_hold(KeyCode::ShiftRight)
            {
                4.0
            } else {
                1.0
            };
            if movement.magnitude2() > 0.0 {
                position += movement.normalize()
                    * config.speed
                    * boost
                    * time.delta_time.as_secs_f32().min(0.1);
            }
        }
    }
    // Controls operate in world space, but scene persistence stores local transforms.
    if let Some(parent) = transform.parent {
        let Ok((_, parent, _, _)) = objects.get(parent) else {
            return;
        };
        if parent
            .scale
            .x
            .abs()
            .min(parent.scale.y.abs())
            .min(parent.scale.z.abs())
            < 0.0001
        {
            return;
        }
        let p = parent.rotation.conjugate() * (position - parent.position);
        transform.position = Vec3::new(
            p.x / parent.scale.x,
            p.y / parent.scale.y,
            p.z / parent.scale.z,
        );
        transform.rotation = parent.rotation.conjugate() * rotation;
    } else {
        transform.position = position;
        transform.rotation = rotation;
    }
}
