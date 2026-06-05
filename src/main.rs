use bevy_app::App;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    color_backtrace::install();
    let (chrome_layer, _) = tracing_chrome::ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(chrome_layer).init();
    App::new().add_plugins(WgpuPbrPlugin).run();
}
