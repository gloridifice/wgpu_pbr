use std::sync::Arc;

use bevy_app::{App, AppExit, Plugin};
use bevy_ecs::prelude::*;
use pollster::block_on;
use winit::{
    application::ApplicationHandler, dpi::PhysicalSize, event::WindowEvent, event_loop::EventLoop,
    window::Window,
};

pub struct WindowAndRenderStatePlugin;

#[derive(Resource, Clone)]
pub struct MainWindow(pub Arc<Window>);

impl Plugin for WindowAndRenderStatePlugin {
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

pub struct MyApplicationHandler {
    app: App,
}

impl ApplicationHandler for MyApplicationHandler {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });
        let window = Arc::new(window);
        let i_width = 1600;
        let i_height = 900;
        let _ = window.request_inner_size(PhysicalSize::new(i_width, i_height));
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        // TODO EguiRenderer and RenderState need to be migrated into there crate
        let render_state = block_on(RenderState::new(&instance, surface, i_width, i_height));

        self.app.insert_resource(render_state);
        self.app.insert_resource(MainWindow(Arc::clone(&window)));

        window.request_redraw();
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
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
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let window = Arc::clone(&self.app.world_mut().resource::<MainWindow>().0);

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
                self.app.world_mut().trigger(ResizeEvent { physical_size });
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
    pub physical_size: PhysicalSize<u32>,
}
