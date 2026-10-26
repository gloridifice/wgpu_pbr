use super::*;
use crate::{control::ControlPlugin, scene::SceneController};
use lentille_core::input::CursorButton;
use std::time::Duration;

fn setup() -> (bevy_app::App, Entity) {
    let mut app = bevy_app::App::new();
    app.init_resource::<Input>()
        .init_resource::<Time>()
        .init_resource::<SceneController>()
        .init_resource::<SceneAssetCache>()
        .init_resource::<SelectedObjects>()
        .add_plugins(ControlPlugin)
        .add_systems(
            bevy_app::PostUpdate,
            lentille_render::transform::sys_update_world_transform,
        );
    app.world_mut().resource_mut::<Input>().window_focused = true;
    app.world_mut().resource_mut::<Time>().delta_time = Duration::from_millis(100);
    app.world_mut()
        .resource_mut::<ControlState>()
        .viewport_active = true;
    let camera = app
        .world_mut()
        .spawn((Camera::new(1.0), MainCamera, CameraController::default()))
        .id();
    (app, camera)
}

fn position(app: &bevy_app::App, camera: Entity) -> Vec3 {
    app.world().get::<Transform>(camera).unwrap().position
}

#[test]
fn flight_requires_viewport_press_but_survives_leaving_viewport_and_stops_on_release_or_blur() {
    let (mut app, camera) = setup();
    app.world_mut()
        .resource_mut::<Input>()
        .hold_keys
        .insert(KeyCode::KeyW);
    app.update();
    assert_eq!(position(&app, camera), Vec3::zero());
    // A press outside the viewport must not become a capture merely by entering it.
    app.world_mut()
        .resource_mut::<ControlState>()
        .viewport_active = false;
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.down_cursor_buttons.insert(CursorButton::Right);
        input.hold_cursor_buttons.insert(CursorButton::Right);
    }
    app.update();
    app.world_mut()
        .resource_mut::<Input>()
        .down_cursor_buttons
        .clear();
    app.world_mut()
        .resource_mut::<ControlState>()
        .viewport_active = true;
    app.update();
    assert_eq!(position(&app, camera), Vec3::zero());
    app.world_mut()
        .resource_mut::<Input>()
        .down_cursor_buttons
        .insert(CursorButton::Right);
    app.update();
    assert!((position(&app, camera).z + 1.0).abs() < 0.001);
    app.world_mut()
        .resource_mut::<Input>()
        .down_cursor_buttons
        .clear();
    app.world_mut()
        .resource_mut::<ControlState>()
        .viewport_active = false;
    app.update();
    assert!((position(&app, camera).z + 2.0).abs() < 0.001);
    app.world_mut()
        .resource_mut::<Input>()
        .hold_cursor_buttons
        .clear();
    app.update();
    assert!((position(&app, camera).z + 2.0).abs() < 0.001);
    app.world_mut()
        .resource_mut::<ControlState>()
        .viewport_active = true;
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.down_cursor_buttons.insert(CursorButton::Right);
        input.hold_cursor_buttons.insert(CursorButton::Right);
    }
    app.update();
    // Wheel changes base speed; Shift boosts it without changing that base.
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.down_cursor_buttons.clear();
        input.hold_keys.insert(KeyCode::ShiftLeft);
        input.hold_keys.insert(KeyCode::KeyE);
        input.wheel_delta = 1.0;
    }
    let before_boost = position(&app, camera);
    app.update();
    let movement = position(&app, camera) - before_boost;
    assert!((movement.magnitude() - 4.8).abs() < 0.001);
    assert!(movement.y > 0.0 && movement.z < 0.0);
    assert!((app.world().resource::<CameraConfig>().speed - 12.0).abs() < 0.001);
    let before = position(&app, camera);
    app.world_mut().resource_mut::<Input>().window_focused = false;
    app.update();
    assert_eq!(position(&app, camera), before);
    assert!(app.world().resource::<ControlState>().mode == NavigationMode::Idle);
}

#[test]
fn focus_frames_selected_subtree_in_narrow_viewport_and_orbit_keeps_target_centered() {
    let (mut app, camera) = setup();
    app.world_mut()
        .get_mut::<CameraState>(camera)
        .unwrap()
        .aspect = 0.5;
    let root = app.world_mut().spawn(Transform::default()).id();
    let child = Transform {
        parent: Some(root),
        position: Vec3::new(6.0, 0.0, 0.0),
        ..Default::default()
    };
    app.world_mut().spawn(child);
    app.world_mut()
        .resource_mut::<SelectedObjects>()
        .entities
        .push(root);
    app.update();
    app.world_mut()
        .resource_mut::<Input>()
        .down_keys
        .insert(KeyCode::KeyF);
    app.update();
    // Two empty-node extents cover [-.5, 6.5] x [-.5, .5] x [-.5, .5].
    let center = Vec3::new(3.0, 0.0, 0.0);
    let focused = position(&app, camera);
    assert!((focused.x - 3.0).abs() < 0.001);
    let projection = app.world().get::<Camera>(camera).unwrap().fovy.to_radians() * 0.5;
    assert!((focused.z - 0.5) * projection.tan() * 0.5 > 3.5);
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.down_keys.clear();
        input.hold_keys.insert(KeyCode::AltLeft);
        input.down_cursor_buttons.insert(CursorButton::Left);
        input.hold_cursor_buttons.insert(CursorButton::Left);
    }
    app.update();
    {
        let mut input = app.world_mut().resource_mut::<Input>();
        input.down_cursor_buttons.clear();
        input.cursor_delta = Vec2::new(150.0, 75.0);
    }
    app.update();
    let transform = app.world().get::<Transform>(camera).unwrap();
    assert!((transform.position - focused).magnitude() > 1.0);
    assert!(
        ((transform.position - center).magnitude() - (focused - center).magnitude()).abs() < 0.001
    );
    assert!(
        transform
            .forward()
            .dot((center - transform.position).normalize())
            > 0.9999
    );
    app.world_mut().resource_mut::<Input>().hold_keys.clear();
    let before = position(&app, camera);
    app.update();
    assert_eq!(position(&app, camera), before);
}
