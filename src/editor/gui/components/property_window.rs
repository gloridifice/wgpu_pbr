use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use egui::{InnerResponse, Pos2, accesskit::Uuid, ahash::HashMap};

use crate::{editor::gui::components::property_window_ui, egui_renderer::EguiRenderer};

pub struct PropertyWindowPlugin;

impl Plugin for PropertyWindowPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(Update, (sys_show, sys_delete))
            .init_resource::<EntityToPropertyWindowIdMap>();
    }
}

pub struct TryCreateEntityPropertyWindowCmd {
    pub pos: egui::Pos2,
    pub entity: Entity,
}

impl Command for TryCreateEntityPropertyWindowCmd {
    fn apply(self, world: &mut World) -> () {
        let should_create = !world
            .resource::<EntityToPropertyWindowIdMap>()
            .map
            .contains_key(&self.entity);
        if should_create {
            world.spawn((
                Name::new("property_window"),
                EntityPropertyWindow {
                    init_pos: self.pos,
                    entity: Some(self.entity),
                    ..Default::default()
                },
                AutoDelete,
            ));
        }
    }
}

/// Tracks which entity is selected for property editing.
#[derive(Resource, Default)]
pub struct EntityToPropertyWindowIdMap {
    /// map from an entity to it's related `property window entity`
    pub map: HashMap<Entity, Entity>,
}

#[derive(Debug, Clone, Component)]
pub struct EntityPropertyWindow {
    pub pinned: bool,
    pub open: bool,
    pub entity: Option<Entity>,
    pub egui_id: egui::Id,
    pub init_pos: Pos2,
}

#[derive(Debug, Clone, Component)]
pub struct AutoDelete;

impl Default for EntityPropertyWindow {
    fn default() -> Self {
        Self {
            pinned: false,
            open: true,
            entity: None,
            egui_id: egui::Id::new(Uuid::new_v4()),
            init_pos: Default::default(),
        }
    }
}

fn sys_delete(
    mut commands: Commands,
    q_windows: Query<(Entity, &EntityPropertyWindow), With<AutoDelete>>,
    mut map: ResMut<EntityToPropertyWindowIdMap>,
) {
    for (id, window) in q_windows.iter() {
        if !window.open {
            commands.entity(id).despawn();
            if let Some(entity) = window.entity.as_ref() {
                map.map.remove(entity);
            }
        }
    }
}

fn sys_show(world: &mut World) {
    let mut q_windows = world.query::<(Entity, &EntityPropertyWindow)>();
    let openned = q_windows
        .iter(world)
        .filter(|(_, it)| it.open && it.entity.is_some())
        .map(|(id, window)| (id, window.clone()))
        .collect::<Vec<_>>();

    let ctx = world.resource::<EguiRenderer>().context().clone();

    for (id, pinnable_window) in openned {
        let (response, mut new_state) = pinnable_window.show(&ctx, world);

        // Unpin when click blank area
        if !new_state.pinned && ctx.input(|it| it.pointer.any_click()) {
            if let Some(InnerResponse { response, .. }) = response {
                if !response.contains_pointer() {
                    new_state.close();
                }
            }
        }

        world.entity_mut(id).insert(new_state);
    }
}

impl EntityPropertyWindow {
    pub fn show(
        mut self,
        ctx: &egui::Context,
        world: &mut World,
    ) -> (Option<InnerResponse<Option<()>>>, EntityPropertyWindow) {
        let Self {
            pinned,
            open,
            entity: Some(entity),
            egui_id,
            init_pos,
        } = &mut self
        else {
            return (None, self);
        };

        let window = egui::Window::new("entity_properties")
            .id(*egui_id)
            .title_bar(false)
            .default_height(400.)
            .default_width(400.)
            .default_pos(*init_pos);

        let response = window.show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.small_button("✖").on_hover_text("Close").clicked() {
                    *open = false;
                }

                let display_name = {
                    let mut ret = format!("#{}", entity.index());
                    if let Some(name) = world.get::<Name>(*entity) {
                        ret.insert_str(0, name.as_str());
                    }
                    ret
                };
                ui.label(&display_name);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pin_button = if *pinned {
                        ui.small_button("📌").on_hover_text("Unpin window")
                    } else {
                        ui.small_button("📍").on_hover_text("Pin window")
                    };

                    if pin_button.clicked() {
                        *pinned = !*pinned;
                    };
                });
            });
            ui.separator();

            property_window_ui(ui, *entity, world);
        });
        (response, self)
    }

    /// Close the window and reset pin state.
    pub fn close(&mut self) {
        self.open = false;
        self.pinned = false;
    }

    /// Toggle the pin flag. When pinned, clicking outside does **not** close the window.
    pub fn toggle_pin(&mut self) {
        self.pinned = !self.pinned;
    }
}
