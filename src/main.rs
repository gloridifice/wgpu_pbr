use std::env;

use bevy_app::App;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    color_backtrace::install();

    // -t to run tracing
    let args = env::args().collect::<Vec<_>>();

    if args.get(1).is_some_and(|it| it == "-t") {
        let (chrome_layer, _guard) = tracing_chrome::ChromeLayerBuilder::new().build();
        tracing_subscriber::registry()
            .with(chrome_layer)
            .with(
                EnvFilter::from_default_env()
                    .add_directive("bevy_ecs=trace".parse().unwrap())
                    .add_directive("wgpu=info".parse().unwrap()),
            )
            .init();
    }

    App::new().add_plugins(WgpuPbrPlugin).run();
}
