use bevy_ecs::{prelude::*, system::RunSystemOnce};
use egui::load::SizedTexture;

use crate::{
    egui_tools::{self, EguiRenderer},
    engine::input::{CursorButton, Input},
    math_type::{Vec2, VectorExt},
    render::{
        self,
        camera::{Camera, CameraConfig},
        light::ParallelLight,
        post_processing::{PostProcessingManager},
        transform::Transform,
        ColorRenderTarget, DepthRenderTarget, RenderTargetSize,
    },
    RenderState,
};

pub enum Pane {
    MainView,
    ControlPanel,
}

struct TreeBehavior<'a> {
    world: &'a mut World,
}

impl<'a> egui_tiles::Behavior<Pane> for TreeBehavior<'a> {
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
                self.world
                    .run_system_once_with(ui, sys_control_panel_ui)
                    .unwrap();
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

fn sys_control_panel_ui(
    InMut(mut ui): InMut<egui::Ui>,
    mut camera_config: ResMut<CameraConfig>,
    cam_single: Single<(
        &mut render::camera::Camera,
        &mut render::camera::CameraController,
        &mut Transform,
    )>,
    light_single: Single<(&ParallelLight, &mut Transform), Without<render::camera::Camera>>,
) {
    let (_, _, mut cam_trans) = cam_single.into_inner();
    let (_, mut light_trans) = light_single.into_inner();
    ui.label("Camera");
    ui.add(egui::widgets::Slider::new(&mut camera_config.speed, 0.5..=10.0).text("Speed"));
    egui_tools::transform_ui(&mut ui, &mut cam_trans);
    ui.separator();
    ui.label("Light");
    egui_tools::transform_ui(ui, &mut light_trans);
}

pub fn sys_egui_tiles(world: &mut World) {
    let mut tree = create_tree();
    world.resource_scope(|world, egui: Mut<EguiRenderer>| {
        let ctx = egui.context();
        egui::SidePanel::left("left_side_panel").show(ctx, |ui| {
            let mut behavior = TreeBehavior { world };
            tree.ui(&mut behavior, ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let id = world.resource::<RenderTargetEguiTexId>();
            let size = ui.available_size();
            if let Some(id) = id.0 {
                let main_view = ui.image(SizedTexture::new(id, size));
                let mut input = world.resource_mut::<Input>();
                for (ec, mc) in vec![
                    (egui::PointerButton::Primary, CursorButton::Left),
                    (egui::PointerButton::Secondary, CursorButton::Right),
                    (egui::PointerButton::Middle, CursorButton::Middle),
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
            let mut target_size = world.resource_mut::<RenderTargetSize>();
            if target_size.height != size.x as u32 || target_size.width != size.y as u32 {
                target_size.height = size.x as u32;
                target_size.width = size.y as u32;
            }
        });
    });
}

#[derive(Resource, Clone, Copy, Default)]
pub struct RenderTargetEguiTexId(Option<egui::TextureId>);

pub fn sys_on_resize_render_target(
    target_size: Res<RenderTargetSize>,
    render_state: Res<RenderState>,
    mut color_target: ResMut<ColorRenderTarget>,
    mut depth_target: ResMut<DepthRenderTarget>,
    mut egui_tex_id: ResMut<RenderTargetEguiTexId>,
    mut egui: ResMut<EguiRenderer>,
    mut camera: Single<&mut Camera>,
    mut post_processing_manager: ResMut<PostProcessingManager>,
) {
    if target_size.is_changed() {
        let device = &render_state.device;
        let config = &render_state.config;
        let width = target_size.width;
        let height = target_size.height;
        color_target.0 = Some(render::create_color_render_target_image(
            width, height, device, config,
        ));
        depth_target.0 = Some(render::create_depth_texture(device, width, height));

        let id = egui.renderer.register_native_texture(
            device,
            &color_target.0.as_ref().unwrap().view,
            wgpu::FilterMode::Linear,
        );
        egui_tex_id.0 = Some(id);
        camera.aspect = height as f32 / width as f32;
        post_processing_manager.resize(width, height, device, config);
    };
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
