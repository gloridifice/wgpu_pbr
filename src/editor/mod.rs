use bevy_app::Plugin;

mod gui;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut bevy_app::App) {}
}
