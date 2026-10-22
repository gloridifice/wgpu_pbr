use std::{env, path::Path};

use bevy_app::App;
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    color_backtrace::install();

    let args = env::args().collect::<Vec<_>>();

    let _gurad: Option<i32> = if args.get(1).is_some_and(|it| it == "-t") {
        let path = Path::new("./build/hello_1.json");
        wgpu_subscriber::initialize_default_subscriber(Some(path));
        // let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new().build();
        // tracing_subscriber::registry()
        //     .with(chrome_layer)
        //     .with(tracing_subscriber::fmt::layer())
        //     .with(EnvFilter::new(
        //         "info,bevy_ecs=trace,wgpu=trace,wgpu_core=trace,wgpu_hal=trace",
        //     ))
        //     .init();
        None
    } else {
        None
    };

    let mut app = App::new();
    app.add_plugins(WgpuPbrPlugin);
    if let Some(index) = args.iter().position(|a| a == "--scene") {
        if let Some(path) = args.get(index + 1) {
            app.world_mut()
                .resource_mut::<wgpu_pbr::scene::SceneController>()
                .request = Some(wgpu_pbr::scene::SceneRequest::Open(path.into()));
        } else {
            eprintln!("--scene requires a path relative to assets/");
            std::process::exit(2);
        }
    }
    app.run();
}
