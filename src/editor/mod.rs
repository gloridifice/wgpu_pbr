use bevy_app::Plugin;

use crate::editor::gui::EditorGuiPlugin;

mod gui;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(EditorGuiPlugin);
    }
}
