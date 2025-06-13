use bevy_app::App;
use bevy_ecs::prelude::*;
use log::info;
use std::sync::Arc;
use wgpu::{Features, Instance, Surface};
use winit::{dpi::PhysicalSize, window::Window};

use crate::window::{ResizeEvent, WindowAndRenderStatePlugin};

mod app;
mod cgmath_ext;
mod editor;
mod egui_tools;
mod macro_utils;
pub mod wgpu_init;

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
pub struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
}

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

impl RenderState {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub async fn new(
        instance: &Instance,
        surface: Surface<'static>,
        width: u32,
        height: u32,
    ) -> RenderState {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let required_features = {
            let mut ret = Features::empty();
            for feat in DEVICE_FEATURES.iter() {
                ret |= *feat;
            }
            ret
        };
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features,
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        info!("Surface format is: '{:?}'.", surface_format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            // determine how to sync
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            surface,
            config,
            size: PhysicalSize { width, height },
        }
    }

    #[allow(unused)]
    fn get_window_extend3d(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.config.width.max(1),
            height: self.config.height.max(1),
            depth_or_array_layers: 1,
        }
    }
}
