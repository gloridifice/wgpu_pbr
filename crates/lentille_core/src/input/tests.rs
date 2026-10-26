use super::*;
use bevy_app::App;
use winit::event::{DeviceId, ElementState};
use winit::window::WindowId;

#[test]
fn raw_motion_accumulates_until_frame_end_and_focus_loss_clears_held_inputs() {
    let mut app = App::new();
    app.add_plugins(InputPlugin);
    app.update();
    let window_event = |window_event| WinitWindowEvent {
        window_id: WindowId::dummy(),
        window_event,
    };
    app.world_mut()
        .trigger(window_event(WindowEvent::Focused(true)));
    for delta in [(2.0, 3.0), (4.0, -1.0)] {
        app.world_mut().trigger(WinitDeviceEvent {
            device_id: DeviceId::dummy(),
            device_event: DeviceEvent::MouseMotion { delta },
        });
    }
    app.world_mut()
        .trigger(window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state: ElementState::Pressed,
            button: MouseButton::Right,
        }));
    let input = app.world().resource::<Input>();
    assert_eq!(input.cursor_delta, Vec2::new(6.0, 2.0));
    assert!(input.is_cursor_button_down(CursorButton::Right));
    app.update();
    let input = app.world().resource::<Input>();
    assert_eq!(input.cursor_delta, Vec2::zero());
    assert!(!input.is_cursor_button_down(CursorButton::Right));
    assert!(input.hold_cursor_buttons.contains(&CursorButton::Right));
    app.world_mut()
        .resource_mut::<Input>()
        .hold_keys
        .insert(KeyCode::KeyW);
    app.world_mut()
        .trigger(window_event(WindowEvent::Focused(false)));
    let input = app.world().resource::<Input>();
    assert!(input.hold_cursor_buttons.is_empty());
    assert!(!input.is_key_hold(KeyCode::KeyW));
    assert!(!input.window_focused);
}
