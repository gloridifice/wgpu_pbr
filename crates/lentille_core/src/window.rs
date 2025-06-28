use std::sync::Arc;

use bevy_app::{App, AppExit, Plugin};
use bevy_ecs::event::Event;
use bevy_ecs::prelude::*;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

pub struct WindowPlugin;

#[derive(Component, Clone, Default, Debug)]
pub struct PrimaryWinodw;

#[derive(Component, Clone, Debug)]
pub struct WinitWindow(pub Arc<Window>);

impl Plugin for WindowPlugin {
    fn build(&self, app: &mut App) {
        app.set_runner(my_runner);
    }
}

pub fn my_runner(app: App) -> AppExit {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    let mut app_handler = MyApplicationHandler { app };
    match event_loop.run_app(&mut app_handler) {
        Ok(_) => AppExit::Success,
        Err(e) => {
            println!("Exit app with Error: {:?}", e);
            AppExit::error()
        }
    }
}

#[derive(Event, Clone)]
pub struct MainWindowCreatedEvent {
    pub id: Entity,
    pub window: Arc<Window>,
}

pub struct MyApplicationHandler {
    app: App,
}

impl ApplicationHandler for MyApplicationHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let id = self
            .app
            .world_mut()
            .spawn((WinitWindow(Arc::clone(&window)), PrimaryWinodw))
            .id();

        self.app.world_mut().trigger(MainWindowCreatedEvent {
            id,
            window: Arc::clone(&window),
        });

        window.request_redraw();
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.app.world_mut().send_event(WinitDeviceEvent {
            device_event: event.clone(),
            device_id,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let world = &mut self.app.world_mut();
        // Find the releated window for request redraw
        let window_arc = world
            .query::<&WinitWindow>()
            .iter(world)
            .find(|it| it.0.id() == window_id);

        let Some(winit_window) = window_arc else {
            return;
        };

        let window = Arc::clone(&winit_window.0);

        match event {
            //Update and Render
            WindowEvent::RedrawRequested => {
                self.app.update();
                window.request_redraw();
            }

            // Close / Exit
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            // Reszie
            WindowEvent::Resized(physical_size) => {
                self.app.world_mut().trigger(ResizeEvent {
                    window_id,
                    physical_size,
                });
            }
            _ => {}
        }
    }
}

#[derive(Event)]
pub struct WinitDeviceEvent {
    pub device_event: winit::event::DeviceEvent,
    pub device_id: winit::event::DeviceId,
}

#[derive(Event)]
pub struct WinitWindowEvent {
    pub window_id: winit::window::WindowId,
    pub window_event: WindowEvent,
}

#[derive(Event)]
pub struct ResizeEvent {
    pub window_id: WindowId,
    pub physical_size: PhysicalSize<u32>,
}
