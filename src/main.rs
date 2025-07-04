use bevy_app::App;
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    color_backtrace::install();
    App::new().add_plugins(WgpuPbrPlugin).run();
}
