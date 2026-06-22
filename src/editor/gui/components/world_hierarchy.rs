use bevy_ecs::prelude::*;
use egui::Ui;
use lentille_render::prelude::Transform;

pub struct WorldHierarchy {}

impl WorldHierarchy {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(&self, ui: &mut Ui, root: Entity, world: &mut World) -> Option<Entity> {
        let mut clicked = None;
        self.tree_recursive(ui, root, world, &mut clicked);
        clicked
    }

    fn tree_recursive(
        &self,
        ui: &mut Ui,
        id: Entity,
        world: &mut World,
        clicked_entity: &mut Option<Entity>,
    ) {
        let display_name = {
            let mut ret = format!(" #{}", id.index());
            if let Some(name) = world.get::<Name>(id) {
                ret.insert_str(0, name.as_str());
            }
            ret
        };

        let children = world
            .get::<Transform>(id)
            .map(|t| t.children.clone())
            .unwrap_or_default();

        let has_children = !children.is_empty();

        if has_children {
            // Entities with children: clickable collapsible tree node.
            let header = egui::CollapsingHeader::new(display_name)
                .default_open(false)
                .show(ui, |ui| {
                    for child_id in &children {
                        self.tree_recursive(ui, *child_id, world, clicked_entity);
                    }
                });
            if header.header_response.clicked() {
                *clicked_entity = Some(id);
            }
        } else {
            // Leaf entities: simple clickable label with indent to align with tree.
            let resp = ui.add(
                egui::Label::new(egui::RichText::new(format!("  {display_name}")).strong())
                    .sense(egui::Sense::click()),
            );
            if resp.clicked() {
                *clicked_entity = Some(id);
            }
        }
    }
}
