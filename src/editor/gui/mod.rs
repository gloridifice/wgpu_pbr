use std::fs::{self};

use bevy_app::{First, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_log::info;
use egui::{
    epaint::text::InsertFontFamily, load::SizedTexture, CentralPanel, PointerButton, Visuals,
};
use egui_wgpu::ScreenDescriptor;
use lentille_core::{
    input::{CursorButton, Input},
    window::{PrimaryWinodw, WinitWindow, WinitWindowEvent},
};
use lentille_render::{
    app_ext::AppExt,
    camera::{Camera, RenderTarget, RenderTargetSize, TargetType},
    prelude::*,
    FrameSets, RenderPreparedStartup, SurfaceState,
};

use components::world_tree;

use crate::egui_renderer::EguiRenderer;

pub mod components;

pub struct EditorGuiPlugin;

impl Plugin for EditorGuiPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(EguiRendererPlugin)
            .init_resource::<EguiConfig>()
            .init_resource::<RenderTargetEguiTexId>()
            .add_systems(RenderPreparedStartup, sys_setup_egui_visual)
            .add_systems(Update, (sys_egui_tiles, sys_on_resize_egui_target));
    }
}

// ===== EguiRenderer plugin =====

pub struct EguiRendererPlugin;

impl Plugin for EguiRendererPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            First,
            sys_begin_frame.run_if(resource_exists::<RenderState>),
        )
        .add_render_system_in_frame_set(FrameSets::Draw, sys_begin_frame)
        .add_render_system_in_frame_set(FrameSets::PostDraw, sys_end_frame_and_draw)
        .init_render_resource::<EguiRenderer>()
        .add_observer(sys_handle_input);
    }
}

/// Only for PrimaryWindow now!
fn sys_begin_frame(
    mut egui_renderer: ResMut<EguiRenderer>,
    window: Single<&WinitWindow, With<PrimaryWinodw>>,
) {
    egui_renderer.begin_frame(&window.0);
}

/// Only for PrimaryWindow now!
fn sys_end_frame_and_draw(
    mut egui_renderer: ResMut<EguiRenderer>,
    rs: Res<RenderState>,
    egui_config: Res<EguiConfig>,
    window: Single<(&WinitWindow, &SurfaceState), With<PrimaryWinodw>>,
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

    egui_renderer.end_frame_and_draw(
        &rs.device,
        &rs.queue,
        &mut encoder,
        &window.0,
        &surface_state
            .surface
            .get_current_texture()
            .unwrap()
            .texture
            .create_view(&Default::default()),
        screen_descriptor,
    );

    rs.queue.submit(std::iter::once(encoder.finish()));
}

fn sys_handle_input(
    trigger: Trigger<WinitWindowEvent>,
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
    MainView,
    ControlPanel,
}

struct TreeBehavior<'a> {
    world: &'a mut World,
}

#[derive(Resource, Clone, Default)]
struct RenderTargetEguiTexId(Option<egui::TextureId>);

impl egui_tiles::Behavior<Pane> for TreeBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::MainView => {
                ui.label("Main View");
            }
            Pane::ControlPanel => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let id_root = self
                        .world
                        .query::<(Entity, &Transform)>()
                        .iter(self.world)
                        .filter_map(|(id, trans)| {
                            if trans.parent.is_none() {
                                Some(id)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    for id in id_root.into_iter() {
                        world_tree(ui, id, self.world);
                    }
                });
            }
        };
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        match pane {
            Pane::MainView => "Main View".into(),
            Pane::ControlPanel => "Control Panel".into(),
        }
    }
}

pub fn sys_egui_tiles(world: &mut World) {
    let mut tree = create_tree();
    world.resource_scope(|world, egui: Mut<EguiRenderer>| {
        let ctx = egui.context();
        egui::SidePanel::left("left_side_panel")
            .default_width(256.)
            .show(ctx, |ui| {
                let mut behavior = TreeBehavior { world };
                tree.ui(&mut behavior, ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            let ids = world.resource::<RenderTargetEguiTexId>();
            let size = ui.available_size();
            if let Some(render_target_egui_tex_id) = ids.0.as_ref() {
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

            let mut target_size = world
                .query_filtered::<&mut RenderTargetSize, With<Camera>>()
                .single_mut(world)
                .unwrap();
            if target_size.height != size.x as u32 || target_size.width != size.y as u32 {
                target_size.height = size.x as u32;
                target_size.width = size.y as u32;
            }
        });
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

fn sys_on_resize_egui_target(
    mut egui_tex_id: ResMut<RenderTargetEguiTexId>,
    mut egui: ResMut<EguiRenderer>,
    camera: Single<&RenderTarget, Changed<RenderTarget>>,
    rs: Res<RenderState>,
) {
    let device = &rs.device;
    if let TargetType::Texture(image) = &camera.target_type {
        egui_tex_id.0 = Some(egui.renderer.register_native_texture(
            device,
            &image.view,
            wgpu::FilterMode::Linear,
        ));
    }
}

fn create_tree() -> egui_tiles::Tree<Pane> {
    let mut tiles = egui_tiles::Tiles::default();

    let mut left_tabs_id_vec = vec![];
    let control_pane = tiles.insert_pane(Pane::ControlPanel);
    let main_view_pane = tiles.insert_pane(Pane::MainView);
    left_tabs_id_vec.push(tiles.insert_vertical_tile(vec![control_pane]));
    left_tabs_id_vec.push(tiles.insert_vertical_tile(vec![main_view_pane]));

    let left_tabs = tiles.insert_tab_tile(left_tabs_id_vec);

    let root = tiles.insert_horizontal_tile(vec![left_tabs]);

    egui_tiles::Tree::new("main_tree", root, tiles)
}
