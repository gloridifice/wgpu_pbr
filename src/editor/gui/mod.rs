use std::fs::{self};

use bevy_app::{First, Plugin, Update};
use bevy_ecs::prelude::*;
use egui::{
    CentralPanel, Color32, Panel, PointerButton, ScrollArea, Visuals,
    epaint::text::InsertFontFamily, load::SizedTexture,
};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use egui_ltreeview::Action;
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
    shadow::csm::{CascadeShadowMapping, CsmConfig},
};

use components::{
    depth_to_rgba::CsmDepthToRgbaConverter, depth_to_rgba::DepthToRgbaConverter,
    texture_preview::TexturePreview,
};

use crate::{
    control::camera::MainCamera,
    editor::gui::components::{
        EditorComponentPlugin,
        property_window::TryCreateEntityPropertyWindowCmd,
        world_hierarchy::{HierarchyEntityQuery, WorldHierarchy},
    },
    egui_renderer::EguiRenderer,
};

pub mod components;

pub struct EditorGuiPlugin;

impl Plugin for EditorGuiPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins((EguiRendererPlugin, EditorComponentPlugin))
            .init_resource::<EguiConfig>()
            .init_resource::<RenderTargetEguiTexId>()
            .init_resource::<DockLayout>()
            .init_resource::<EditorThemeConfig>()
            .init_resource::<CsmPreviewRefreshState>()
            .add_systems(RenderPreparedStartup, sys_setup_egui_visual_theme)
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

/// Combined theme: egui native style + egui_dock style, both persisted together.
#[derive(serde::Serialize, serde::Deserialize)]
struct EditorTheme {
    egui_style: egui::Style,
    dock_style: egui_dock::Style,
}

/// Holds the editor theme, visibility, and baseline snapshots for reset.
#[derive(Resource)]
struct EditorThemeConfig {
    pub egui_style: egui::Style,
    pub dock_style: egui_dock::Style,
    pub visible: bool,
    pub initialized: bool,
    pub egui_baseline_style: egui::Style,
    pub dock_baseline_style: egui_dock::Style,
    pub dock_default_style: egui_dock::Style,
}

impl Default for EditorThemeConfig {
    fn default() -> Self {
        Self {
            egui_style: egui::Style::default(),
            dock_style: egui_dock::Style::default(),
            visible: false,
            initialized: false,
            egui_baseline_style: egui::Style::default(),
            dock_baseline_style: egui_dock::Style::default(),
            dock_default_style: egui_dock::Style::default(),
        }
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        // Fallback width used before the first frame (actual width unknown).
        Self(create_dock_state(1920.0))
    }
}

/// Controls when CSM depth textures are converted to RGBA for preview.
///
/// `refresh_once` is set by the "Refresh Once" button and consumed next frame.
/// `continuous` is toggled by the "Continuous" button — when true the preview
/// is refreshed every frame.
#[derive(Resource, Default)]
struct CsmPreviewRefreshState {
    continuous: bool,
    refresh_once: bool,
}

impl TabViewer for DockTabViewer<'_> {
    type Tab = Pane;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match tab {
            Pane::RightPanel => "🗺️ CSM Preview".into(),
            Pane::WorldPanel => "📋 Control Panel".into(),
            Pane::Scene => "🎬 Scene".into(),
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

                ScrollArea::vertical().show(ui, |ui| {
                    // --- CSM Settings ---
                    egui::CollapsingHeader::new("⚙️ CSM Settings")
                        .default_open(true)
                        .show(ui, |ui| {
                            let mut config_query = world.query::<&mut CsmConfig>();
                            if let Ok(mut config) = config_query.single_mut(world) {
                                ui.horizontal(|ui| {
                                    ui.label("Linear/Log Factor:");
                                    ui.add(
                                        egui::Slider::new(&mut config.linear_log_factor, 0.0..=1.0)
                                            .text(""),
                                    );
                                });
                            } else {
                                ui.colored_label(Color32::GRAY, "No CSM config");
                            }
                        });

                    // --- CSM Preview Refresh Controls ---
                    let has_csm = world
                        .query::<&CascadeShadowMapping>()
                        .iter(world)
                        .next()
                        .is_some();
                    if has_csm {
                        ui.separator();
                        egui::CollapsingHeader::new("🔄 CSM Preview Refresh")
                            .default_open(true)
                            .show(ui, |ui| {
                                if let Some(mut state) =
                                    world.get_resource_mut::<CsmPreviewRefreshState>()
                                {
                                    ui.horizontal(|ui| {
                                        if ui.button("🔄 Refresh Once").clicked() {
                                            state.refresh_once = true;
                                        }
                                        let label = if state.continuous {
                                            "🔁 Continuous: ON"
                                        } else {
                                            "🔁 Continuous: OFF"
                                        };
                                        if ui.button(label).clicked() {
                                            state.continuous = !state.continuous;
                                        }
                                    });
                                }
                            });
                    }

                    // --- CSM cascade layers ---
                    let converter_count = world
                        .query::<&CsmDepthToRgbaConverter>()
                        .iter(world)
                        .count();
                    if converter_count > 0 {
                        ui.separator();
                        egui::CollapsingHeader::new("📚 CSM Depth Layers")
                            .default_open(true)
                            .show(ui, |ui| {
                                let mut converter_query =
                                    world.query::<(&mut CsmDepthToRgbaConverter, &Name)>();
                                for (mut converter, name) in converter_query.iter_mut(world) {
                                    ui.collapsing(name.as_str(), |ui| {
                                        for (i, output) in
                                            converter.outputs_mut().iter_mut().enumerate()
                                        {
                                            ui.label(format!("Cascade {}", i));
                                            ui.label(format!(
                                                "Depth format: {:?}",
                                                output.original_format
                                            ));
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
                            });
                    }
                });
            }
            Pane::WorldPanel => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // --- Performance section ---
                    egui::CollapsingHeader::new("📊 Performance")
                        .default_open(true)
                        .show(ui, |ui| {
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
                            } else {
                                ui.colored_label(Color32::GRAY, "No time data");
                            }
                        });

                    ui.separator();

                    // --- Scene Hierarchy section ---
                    let mut query_state = world.query::<HierarchyEntityQuery>();
                    let query = query_state.query(world);
                    let root_entities = query.iter().filter_map(|(id, _, trans)| {
                        if trans.parent.is_none() {
                            Some(id)
                        } else {
                            None
                        }
                    });

                    let (_, actions) =
                        WorldHierarchy::new().show(ui, &root_entities.collect(), &query);

                    for action in actions {
                        match action {
                            Action::SetSelected(list) => {
                                for entity in list {
                                    if let Some(pos) =
                                        egui_renderer.context().input(|i| i.pointer.latest_pos())
                                    {
                                        TryCreateEntityPropertyWindowCmd { pos: pos, entity }
                                            .apply(world);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
            Pane::Scene => {
                let size = ui.available_size();
                if let Some(ids) = world.get_resource::<RenderTargetEguiTexId>()
                    && let Some(render_target_egui_tex_id) = ids.0.as_ref()
                {
                    let main_view = ui.image(SizedTexture::new(*render_target_egui_tex_id, size));
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
    {
        let has_csm = world
            .query::<&CascadeShadowMapping>()
            .iter(world)
            .next()
            .is_some();
        if has_csm {
            let should_refresh = world
                .get_resource::<CsmPreviewRefreshState>()
                .is_some_and(|s| s.continuous || s.refresh_once);
            if should_refresh {
                convert_csm_depth_to_rgba(world);
                if let Some(mut s) = world.get_resource_mut::<CsmPreviewRefreshState>() {
                    s.refresh_once = false;
                }
            }
        }
    }

    // Snapshot a raw pointer to the device — RenderState lives forever,
    // so this pointer is valid for the lifetime of the app.
    let device_ptr: *const wgpu::Device = {
        let rs = world.resource::<RenderState>();
        &rs.device as *const wgpu::Device
    };

    // Collect status-bar data before entering resource_scope.
    let status_fps = world.get_resource::<Time>().map(|t| t.fps).unwrap_or(0.0);
    let status_frame_ms = world
        .get_resource::<Time>()
        .map(|t| t.frame_time_ms)
        .unwrap_or(0.0);
    let status_entity_count = world.query::<Entity>().iter(world).count();

    // Use Cell so the menu-bar closure can signal the dock-area closure.
    let reset_layout = std::cell::Cell::new(false);
    let show_about = std::cell::Cell::new(false);
    let toggle_style_window = std::cell::Cell::new(false);

    world.resource_scope(|world, mut egui: Mut<EguiRenderer>| {
        let ctx = egui.context().clone();
        let mut ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("editor_root"),
            egui::UiBuilder::new().max_rect(ctx.content_rect()),
        );

        // ---- Menu Bar ----
        Panel::top("menu_bar").show_inside(&mut ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {
                    if ui.button("🔄 Reset Layout").clicked() {
                        reset_layout.set(true);
                        ui.close();
                    }
                    if ui.button("🎨 Dock Style").clicked() {
                        toggle_style_window.set(true);
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        show_about.set(true);
                        ui.close();
                    }
                });
            });
        });

        // ---- Status Bar ----
        Panel::bottom("status_bar").show_inside(&mut ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "🖥️  FPS: {:.1}  |  Frame: {:.2} ms",
                    status_fps, status_frame_ms
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Entities: {}  ", status_entity_count));
                });
            });
        });

        // ---- Editor Theme Config (init + toggle + editor window) ----
        {
            let mut cfg = world.resource_mut::<EditorThemeConfig>();

            // Lazy-init on the first frame: try loading saved theme, fall back to defaults.
            if !cfg.initialized {
                let current_style = ctx.global_style().as_ref().clone();
                cfg.egui_baseline_style = current_style.clone();

                let dock_theme = egui_dock::Style::from_egui(&current_style);
                cfg.dock_baseline_style = dock_theme.clone();
                cfg.dock_default_style = egui_dock::Style::default();

                // Try loading saved theme; if missing/invalid, use current live defaults.
                if let Some(saved) = load_editor_theme() {
                    cfg.egui_style = saved.egui_style;
                    cfg.dock_style = saved.dock_style;
                } else {
                    cfg.egui_style = current_style;
                    cfg.dock_style = {
                        let mut s = dock_theme;
                        s.tab_bar.height = 28.0;
                        s
                    };
                }
                cfg.initialized = true;
            }

            // Apply the egui style to the context every frame for live preview.
            ctx.set_global_style(cfg.egui_style.clone());

            // Toggle visibility via menu signal.
            if toggle_style_window.take() {
                cfg.visible = !cfg.visible;
            }

            // Render the style editor window.
            // We copy state out to avoid overlapping borrows between .open() and .show().
            if cfg.visible {
                let egui_baseline = cfg.egui_baseline_style.clone();
                let dock_baseline = cfg.dock_baseline_style.clone();
                let dock_default = cfg.dock_default_style.clone();
                let mut egui_style = cfg.egui_style.clone();
                let mut dock_style = cfg.dock_style.clone();
                let mut win_visible = cfg.visible;
                let mut save_requested = false;

                egui::Window::new("🎨 Editor Theme")
                    .open(&mut win_visible)
                    .default_width(420.0)
                    .default_height(700.0)
                    .show(&ctx, |ui| {
                        save_requested = components::dock_style_editor::show(
                            ui,
                            &mut egui_style,
                            &egui_baseline,
                            &mut dock_style,
                            &dock_baseline,
                            &dock_default,
                        );
                    });

                if save_requested {
                    save_editor_theme(&EditorTheme {
                        egui_style: egui_style.clone(),
                        dock_style: dock_style.clone(),
                    });
                }
                cfg.egui_style = egui_style;
                cfg.dock_style = dock_style;
                cfg.visible = win_visible;
            }
        }

        // ---- About Window ----
        let mut show_about_window = show_about.take();
        if show_about_window {
            egui::Window::new("About wgpu_pbr Editor")
                .open(&mut show_about_window)
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label("wgpu_pbr — Real-time PBR Renderer");
                    ui.separator();
                    ui.label("A deferred rendering engine with PBR materials,");
                    ui.label("IBL lighting, CSM shadows, and an egui-based editor.");
                });
        }

        // ---- Central Area: DockArea ----
        let content_width = ctx.content_rect().width();
        CentralPanel::default().show_inside(&mut ui, |ui| {
            // Clone style before entering DockLayout resource_scope to avoid
            // conflicting borrows with DockTabViewer's mutable world reference.
            let dock_style = world.resource::<EditorThemeConfig>().dock_style.clone();

            world.resource_scope(|world, mut dock: Mut<DockLayout>| {
                if reset_layout.take() {
                    *dock = DockLayout(create_dock_state(content_width));
                }
                let mut viewer = DockTabViewer {
                    world,
                    egui_renderer: &mut egui,
                    device: device_ptr,
                };
                DockArea::new(&mut dock.0)
                    .style(dock_style)
                    .show_inside(ui, &mut viewer);
            });
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

fn sys_setup_egui_visual_theme(egui: ResMut<EguiRenderer>) {
    let visual = Visuals::dark();
    let ctx = egui.context();

    //visual.widgets.noninteractive.bg_stroke.width = 0.0;

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

fn save_editor_theme(theme: &EditorTheme) {
    let path = AssetPath::Assets("egui_themes/default.ron".to_string()).final_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match ron::ser::to_string_pretty(theme, ron::ser::PrettyConfig::default()) {
        Ok(s) => {
            if let Err(e) = fs::write(&path, s) {
                bevy_log::error!("Failed to write editor theme to {}: {}", path, e);
            } else {
                bevy_log::info!("Editor theme saved to {}", path);
            }
        }
        Err(e) => bevy_log::error!("Failed to serialize editor theme: {}", e),
    }
}

fn load_editor_theme() -> Option<EditorTheme> {
    let path = AssetPath::Assets("egui_themes/default.ron".to_string()).final_path();
    let data = fs::read_to_string(&path).ok()?;
    match ron::from_str(&data) {
        Ok(theme) => {
            bevy_log::info!("Editor theme loaded from {}", path);
            Some(theme)
        }
        Err(e) => {
            bevy_log::warn!("Failed to deserialize editor theme from {}: {}", path, e);
            None
        }
    }
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

fn create_dock_state(total_width: f32) -> DockState<Pane> {
    /// Desired pixel widths for side panels.
    const LEFT_PANEL_PX: f32 = 280.0;
    const RIGHT_PANEL_PX: f32 = 320.0;

    // Convert pixel widths to ratios of the total available width.
    let left_ratio = (LEFT_PANEL_PX / total_width).clamp(0.10, 0.35);
    let right_ratio = (RIGHT_PANEL_PX / total_width).clamp(0.10, 0.40);
    let center_ratio = 1.0 - left_ratio - right_ratio;

    // If the window is too narrow, scale side panels down proportionally
    // while keeping the center at least 25% of the total.
    let (left_ratio, right_ratio) = if center_ratio < 0.25 {
        let scale = 0.75 / (left_ratio + right_ratio);
        (left_ratio * scale, right_ratio * scale)
    } else {
        (left_ratio, right_ratio)
    };

    let mut state = DockState::new(vec![Pane::Scene]);
    let surface = state.main_surface_mut();

    // Split left: left_ratio goes to WorldPanel, remaining (1-left_ratio) stays with Scene.
    let [_root, _left] = surface.split_left(NodeIndex::root(), left_ratio, vec![Pane::WorldPanel]);

    // Remaining width after left split = 1.0 - left_ratio.
    // To allocate right_ratio of the *total* width, we need:
    //   right_share_of_remaining = right_ratio / (1.0 - left_ratio)
    // split_right keeps `ratio` for existing content (Scene), gives `1-ratio` to new (Right).
    // So Scene keeps: 1.0 - right_ratio / (1.0 - left_ratio) of the remaining.
    let scene_share_of_remaining = 1.0 - right_ratio / (1.0 - left_ratio);
    let [_root2, _right] = surface.split_right(
        NodeIndex::root(),
        scene_share_of_remaining,
        vec![Pane::RightPanel],
    );

    state
}
