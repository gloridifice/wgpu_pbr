use std::{env, path::Path};

use bevy_app::App;
use wgpu_pbr::WgpuPbrPlugin;

fn main() {
    color_backtrace::install();

    let args = env::args().collect::<Vec<_>>();

    let legacy_trace = args.iter().any(|arg| arg == "-t");
    if cfg!(feature = "profile-tracy") && legacy_trace {
        eprintln!("-t cannot be combined with profile-tracy; connect Tracy instead");
        std::process::exit(2);
    }

    if legacy_trace {
        let path = Path::new("./build/hello_1.json");
        wgpu_subscriber::initialize_default_subscriber(Some(path));
    }

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
