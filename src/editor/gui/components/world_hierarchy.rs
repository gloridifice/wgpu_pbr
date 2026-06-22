use bevy_ecs::prelude::*;
use egui::{Color32, Response, Stroke, Ui};
use egui_ltreeview::{IndentHintStyle, TreeView, TreeViewBuilder};
use lentille_render::prelude::Transform;

pub struct WorldHierarchy {}

pub type HierarchyEntityQuery<'a> = (Entity, Option<&'a Name>, &'a Transform);

impl WorldHierarchy {
    pub fn new() -> Self {
        Self {}
    }

    pub fn show(
        &self,
        ui: &mut Ui,
        root: &Vec<Entity>,
        query: &Query<HierarchyEntityQuery>,
    ) -> (Response, Vec<egui_ltreeview::Action<Entity>>) {
        ui.visuals_mut().widgets.noninteractive.bg_stroke = Stroke {
            width: 1.0,
            color: Color32::from_gray(42),
        };
        TreeView::new(ui.make_persistent_id("Hierarchy"))
            .indent_hint_style(IndentHintStyle::Hook)
            .override_striped(Some(true))
            .show(ui, |builder| {
                builder.dir(Entity::PLACEHOLDER, "World");
                for id in root {
                    self.build_tree(*id, query, builder);
                }
                builder.close_dir();
            })
    }

    fn build_tree(
        &self,
        id: Entity,
        query: &Query<HierarchyEntityQuery>,
        builder: &mut TreeViewBuilder<'_, Entity>,
    ) {
        let Ok((id, name, transform)) = query.get(id) else {
            return;
        };

        let display_name = if let Some(name) = name {
            format!("{} #{}", &name, id.index())
        } else {
            format!("#{}", id.index())
        };

        let children = &transform.children;

        if !children.is_empty() {
            builder.dir(id, &display_name);
            for child_id in children {
                self.build_tree(*child_id, query, builder);
            }
            builder.close_dir();
        } else {
            builder.leaf(id, &display_name);
        }
    }
}
