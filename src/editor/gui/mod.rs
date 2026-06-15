use std::fs::{self};

use bevy_app::{First, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_log::info;
use egui::{
    Color32, LayerId, PointerButton, ScrollArea, Visuals, epaint::text::InsertFontFamily,
    load::SizedTexture,
};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use egui_wgpu::ScreenDescriptor;
use lentille_core::{
    input::{CursorButton, Input},
    time::Time,
    window::{PrimaryWindow, WinitWindow, WinitWindowEvent},
};
use lentille_render::{
    FrameSets, RenderPreparedStartup, SurfaceState,
    app_ext::AppExt,
    camera::{RenderTarget, RenderTargetResizedEvent, RenderTargetSize, TargetType},
    prelude::*,
    shadow_mapping::{
        ShadowMap,
        csm::{CascadeShadowMapping, CsmConfig},
    },
};

use components::{
    depth_to_rgba::CsmDepthToRgbaConverter, depth_to_rgba::DepthToRgbaConverter,
    texture_preview::TexturePreview, world_tree,
};

use crate::{control::camera::MainCamera, egui_renderer::EguiRenderer};

pub mod components;

pub struct EditorGuiPlugin;

impl Plugin for EditorGuiPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(EguiRendererPlugin)
            .init_resource::<EguiConfig>()
            .init_resource::<RenderTargetEguiTexId>()
            .init_resource::<DockLayout>()
            .add_systems(RenderPreparedStartup, sys_setup_egui_visual)
            .add_systems(Update, sys_egui_dock)
            .add_observer(sys_on_resize_scene_render_target);
    }
}

// ===== EguiRenderer plugin =====

pub struct EguiRendererPlugin;

impl Plugin for EguiRendererPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(First, sys_begin_frame)
            .add_render_system_in_frame_set(FrameSets::Present, sys_end_frame_and_draw)
            .init_render_resource::<EguiRenderer>()
            .add_observer(sys_handle_input);
    }
}

/// Only for PrimaryWindow now!
fn sys_begin_frame(
    mut egui_renderer: ResMut<EguiRenderer>,
    window: Single<&WinitWindow, With<PrimaryWindow>>,
) {
    egui_renderer.begin_frame(&window.0);
}

/// Only for PrimaryWindow now!
fn sys_end_frame_and_draw(
    mut egui_renderer: ResMut<EguiRenderer>,
    rs: Res<RenderState>,
    egui_config: Res<EguiConfig>,
    window: Single<(&WinitWindow, &SurfaceState), With<PrimaryWindow>>,
) {
    let (window, surface_state) = window.into_inner();

    let mut encoder = rs
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Egui"),
        });

    let screen_descriptor = ScreenDescriptor {
        size_in_pixels: [surface_state.config.width, surface_state.config.height],
        pixels_per_point: window.0.scale_factor() as f32 * egui_config.egui_scale_factor,
    };

    let surface_texture = match surface_state.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        status => {
            bevy_log::error!("Failed to acquire surface texture: {:?}", status);
            return;
        }
    };
    let view = surface_texture.texture.create_view(&Default::default());

    egui_renderer.end_frame_and_draw(
        &rs.device,
        &rs.queue,
        &mut encoder,
        &window.0,
        &view,
        screen_descriptor,
    );

    rs.queue.submit(std::iter::once(encoder.finish()));

    surface_texture.present();
}

fn sys_handle_input(
    trigger: On<WinitWindowEvent>,
    mut egui_renderer: ResMut<EguiRenderer>,
    q_window: Query<&WinitWindow>,
) {
    let Some(window) = q_window.iter().find(|it| it.0.id() == trigger.window_id) else {
        return;
    };

    egui_renderer.handle_input(&window.0, &trigger.window_event);
}

// End ===== EguiRenderer plugin =====

#[derive(Resource)]
pub struct EguiConfig {
    pub egui_scale_factor: f32,
}

impl Default for EguiConfig {
    fn default() -> Self {
        Self {
            egui_scale_factor: 0.8,
        }
    }
}

pub enum Pane {
    RightPanel,
    Scene,
    WorldPanel,
}

struct DockTabViewer<'a> {
    world: &'a mut World,
    egui_renderer: &'a mut EguiRenderer,
    device: *const wgpu::Device,
}

#[derive(Resource, Clone, Default)]
struct RenderTargetEguiTexId(Option<egui::TextureId>);

#[derive(Resource)]
struct DockLayout(DockState<Pane>);

impl Default for DockLayout {
    fn default() -> Self {
        Self(create_dock_state())
    }
}

/// Cached state for previewing the single [`ShadowMap`] depth texture.
#[derive(Resource)]
struct ShadowMapPreviewState {
    converter: DepthToRgbaConverter,
    rgba_tex: Option<wgpu::Texture>,
    rgba_view: Option<wgpu::TextureView>,
    preview: TexturePreview,
    width: u32,
    height: u32,
}

impl TabViewer for DockTabViewer<'_> {
    type Tab = Pane;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Pane::RightPanel => "CSM Preview".into(),
            Pane::WorldPanel => "Control Panel".into(),
            Pane::Scene => "Scene".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let Self {
            world,
            egui_renderer,
            device,
        } = self;
        match tab {
            Pane::RightPanel => {
                let device: &wgpu::Device = unsafe { &**device };

                {
                    let mut config_query = world.query::<&mut CsmConfig>();
                    if let Ok(mut config) = config_query.single_mut(world) {
                        ui.add(egui::Slider::new(&mut config.linear_log_factor, 0.0..=1.0));
                    }
                }

                ScrollArea::vertical().show(ui, |ui| {
                    // --- ShadowMap single preview ---
                    if let Some(mut state) = world.get_resource_mut::<ShadowMapPreviewState>() {
                        let sw = state.width;
                        let sh = state.height;
                        let rgba_view = state.rgba_view.take();
                        if let Some(ref rgba_view) = rgba_view {
                            ui.collapsing("ShadowMap", |ui| {
                                state.preview.show_view(
                                    ui,
                                    &mut egui_renderer.renderer,
                                    device,
                                    rgba_view,
                                    wgpu::Extent3d {
                                        width: sw,
                                        height: sh,
                                        depth_or_array_layers: 1,
                                    },
                                    wgpu::TextureFormat::Rgba8Unorm,
                                );
                            });
                            ui.separator();
                        }
                        state.rgba_view = rgba_view;
                    }

                    // --- CSM cascade layers ---
                    ui.colored_label(Color32::LIGHT_YELLOW, "CSM Depth Layers");
                    ui.separator();

                    let mut has_preview = false;
                    let mut converter_query =
                        world.query::<(&mut CsmDepthToRgbaConverter, &Name)>();
                    for (mut converter, name) in converter_query.iter_mut(world) {
                        has_preview = true;
                        ui.collapsing(name.as_str(), |ui| {
                            for (i, output) in converter.outputs_mut().iter_mut().enumerate() {
                                ui.label(format!("Cascade {}", i));
                                ui.label(format!("Depth format: {:?}", output.original_format));
                                output.preview.size(128., 128.).show_view(
                                    ui,
                                    &mut egui_renderer.renderer,
                                    device,
                                    &output.rgba_view,
                                    wgpu::Extent3d {
                                        width: output.width,
                                        height: output.height,
                                        depth_or_array_layers: 1,
                                    },
                                    wgpu::TextureFormat::Rgba8Unorm,
                                );
                            }
                        });
                    }
                    if !has_preview {
                        ui.colored_label(Color32::GRAY, "No depth data available");
                    }
                });
            }
            Pane::WorldPanel => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(time) = world.get_resource::<Time>() {
                        ui.horizontal(|ui| {
                            ui.label("FPS:");
                            ui.colored_label(
                                egui::Color32::LIGHT_GREEN,
                                format!("{:.1}", time.fps),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label("Frame:");
                            ui.colored_label(
                                egui::Color32::LIGHT_GREEN,
                                format!("{:.2} ms", time.frame_time_ms),
                            );
                        });
                        ui.separator();
                    }

                    let id_root = world
                        .query::<(Entity, &Transform)>()
                        .iter(world)
                        .filter_map(|(id, trans)| {
                            if trans.parent.is_none() {
                                Some(id)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    for id in id_root.into_iter() {
                        world_tree(ui, id, world);
                    }
                });
            }
            Pane::Scene => {
                let size = ui.available_size();
                if let Some(ids) = world.get_resource::<RenderTargetEguiTexId>() {
                    if let Some(render_target_egui_tex_id) = ids.0.as_ref() {
                        let main_view =
                            ui.image(SizedTexture::new(*render_target_egui_tex_id, size));
                        let mut input = world.resource_mut::<Input>();
                        for (ec, mc) in [
                            (PointerButton::Primary, CursorButton::Left),
                            (PointerButton::Secondary, CursorButton::Right),
                            (PointerButton::Middle, CursorButton::Middle),
                        ] {
                            if main_view.clicked_by(ec) {
                                input.down_cursor_buttons.insert(mc);
                            }
                        }
                        input.cursor_position = main_view
                            .hover_pos()
                            .map(|it| Vec2::new(it.x, it.y))
                            .unwrap_or(Vec2::zero());
                    }
                }

                if let Ok(mut target_size) = world
                    .query_filtered::<&mut RenderTargetSize, With<MainCamera>>()
                    .single_mut(world)
                {
                    let new_w = size.x as u32;
                    let new_h = size.y as u32;
                    if target_size.width != new_w || target_size.height != new_h {
                        target_size.width = new_w;
                        target_size.height = new_h;
                    }
                }
            }
        };
    }
}

pub fn sys_egui_dock(world: &mut World) {
    // Convert depth textures to RGBA so egui can display them
    // (depth-format textures cannot be sampled by egui_wgpu directly).
    if world.contains_resource::<ShadowMap>() {
        convert_shadow_map_depth_to_rgba(world);
    }
    {
        let has_csm = world
            .query::<&CascadeShadowMapping>()
            .iter(world)
            .next()
            .is_some();
        if has_csm {
            convert_csm_depth_to_rgba(world);
        }
    }

    // Snapshot a raw pointer to the device — RenderState lives forever,
    // so this pointer is valid for the lifetime of the app.
    let device_ptr: *const wgpu::Device = {
        let rs = world.resource::<RenderState>();
        &rs.device as *const wgpu::Device
    };

    world.resource_scope(|world, mut egui: Mut<EguiRenderer>| {
        let mut ui = {
            let ctx = egui.context().clone();
            egui::Ui::new(
                ctx.clone(),
                egui::Id::new("dock_ui"),
                egui::UiBuilder::new()
                    .layer_id(LayerId::background())
                    .max_rect(ctx.content_rect()),
            )
        };

        world.resource_scope(|world, mut dock: Mut<DockLayout>| {
            let mut viewer = DockTabViewer {
                world,
                egui_renderer: &mut *egui,
                device: device_ptr,
            };
            DockArea::new(&mut dock.0).show_inside(&mut ui, &mut viewer);
        });
    });
}

fn convert_csm_depth_to_rgba(world: &mut World) {
    let mut csm_data: Vec<(
        Entity,
        Vec<std::sync::Arc<TexView2D<SampleDepth>>>,
        u32,
        u32,
        wgpu::TextureFormat,
    )> = Vec::new();
    {
        let mut query = world.query::<(Entity, &CascadeShadowMapping)>();
        for (entity, csm) in query.iter(world) {
            let views: Vec<_> = csm.layers.iter().map(|l| l.view.clone()).collect();
            let size = csm.shadow_maps.size();
            csm_data.push((
                entity,
                views,
                size.width,
                size.height,
                csm.shadow_maps.format(),
            ));
        }
    }

    if csm_data.is_empty() {
        return;
    }

    let (device_ptr, queue_ptr): (*const wgpu::Device, *const wgpu::Queue) = {
        let rs = world.resource::<RenderState>();
        (
            &rs.device as *const wgpu::Device,
            &rs.queue as *const wgpu::Queue,
        )
    };

    for (entity, depth_refs, w, h, format) in &csm_data {
        let mut entity_mut = world.entity_mut(*entity);
        if !entity_mut.contains::<CsmDepthToRgbaConverter>() {
            let device: &wgpu::Device = unsafe { &*device_ptr };
            entity_mut.insert(CsmDepthToRgbaConverter::new(device));
        }

        let device: &wgpu::Device = unsafe { &*device_ptr };
        let queue: &wgpu::Queue = unsafe { &*queue_ptr };
        let depth_views: Vec<&wgpu::TextureView> = depth_refs.iter().map(|v| v.view()).collect();

        let mut converter = entity_mut.get_mut::<CsmDepthToRgbaConverter>().unwrap();
        converter.convert(device, queue, &depth_views, *w, *h, *format);
    }
}

fn convert_shadow_map_depth_to_rgba(world: &mut World) {
    if !world.contains_resource::<ShadowMap>() {
        return;
    }

    if !world.contains_resource::<ShadowMapPreviewState>() {
        let converter = {
            let rs = world.resource::<RenderState>();
            DepthToRgbaConverter::new(&rs.device)
        };
        world.insert_resource(ShadowMapPreviewState {
            converter,
            rgba_tex: None,
            rgba_view: None,
            preview: TexturePreview::new(),
            width: 0,
            height: 0,
        });
    }

    world.resource_scope(|world, mut state: Mut<ShadowMapPreviewState>| {
        let shadow_map = world.resource::<ShadowMap>();
        let rs = world.resource::<RenderState>();
        let device = &rs.device;
        let queue = &rs.queue;

        let tex = &shadow_map.image.texture;
        let size = tex.size();
        let w = size.width;
        let h = size.height;

        if state
            .rgba_tex
            .as_ref()
            .map_or(true, |t| t.size().width != w || t.size().height != h)
        {
            let out_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("shadow_map depth_to_rgba output"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let out_view = out_tex.create_view(&Default::default());
            state.rgba_tex = Some(out_tex);
            state.rgba_view = Some(out_view);
            state.width = w;
            state.height = h;
            state.preview.invalidate();
        }

        if let Some(ref output_view) = state.rgba_view {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            state.converter.convert_to(
                device,
                &mut encoder,
                &shadow_map.image.view,
                output_view,
                w,
                h,
            );
            queue.submit(std::iter::once(encoder.finish()));
        }
    });
}

fn sys_setup_egui_visual(egui: ResMut<EguiRenderer>) {
    info!("Render prepared");
    let mut visual = Visuals::dark();
    let ctx = egui.context();

    visual.widgets.noninteractive.bg_stroke.width = 0.0;
    ctx.set_visuals(visual);

    let font_data =
        fs::read(AssetPath::Assets("fonts/MiSans-Normal.ttf".to_string()).final_path()).unwrap();
    ctx.add_font(egui::epaint::text::FontInsert::new(
        "MiSans",
        egui::FontData::from_owned(font_data),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::epaint::text::FontPriority::Highest,
        }],
    ));
}

fn sys_on_resize_scene_render_target(
    event: On<RenderTargetResizedEvent>,
    mut egui_tex_id: ResMut<RenderTargetEguiTexId>,
    q_camera: Query<&RenderTarget, With<MainCamera>>,
    mut egui: ResMut<EguiRenderer>,
    rs: Res<RenderState>,
) {
    let device = &rs.device;
    if let Ok(RenderTarget {
        target_type: TargetType::Texture(image),
        ..
    }) = &q_camera.get(event.render_target_entity)
    {
        let view = match lentille_render::camera::linear_view_format_of(image.texture.format()) {
            Some(linear_format) => {
                image
                    .texture
                    .texture()
                    .create_view(&wgpu::TextureViewDescriptor {
                        format: Some(linear_format),
                        ..Default::default()
                    })
            }
            None => image
                .texture
                .texture()
                .create_view(&wgpu::TextureViewDescriptor::default()),
        };
        egui_tex_id.0 = Some(egui.renderer.register_native_texture(
            device,
            &view,
            wgpu::FilterMode::Linear,
        ));
    }
}

fn create_dock_state() -> DockState<Pane> {
    // Main area = Scene. Split left 1/4 for WorldPanel, right 1/4 for CSM Preview.
    // Original share ratio: 1:2:1
    let mut state = DockState::new(vec![Pane::Scene]);
    let surface = state.main_surface_mut();
    let [_root, _left] = surface.split_left(NodeIndex::root(), 0.25, vec![Pane::WorldPanel]);
    // Remaining width is 0.75 of total. To get 0.25 of total for right panel,
    // split 1/3 of remaining (0.75 * 1/3 = 0.25).
    let [_root2, _right] =
        surface.split_right(NodeIndex::root(), 2.0 / 3.0, vec![Pane::RightPanel]);
    state
}
