use bevy_app::App;
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    App::new().add_plugins(WgpuPbrPlugin).run();
}
