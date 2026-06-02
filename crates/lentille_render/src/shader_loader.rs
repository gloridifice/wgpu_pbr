use std::{borrow::Cow, fs, path::Path};

use bevy_ecs::prelude::*;
use wesl::{StandardResolver, Wesl};
use wgpu::ShaderSource;

use crate::{RenderState, asset::AssetPath};

#[derive(Resource)]
pub struct ShaderLoader {
    pub root: String,
    pub wesl: Wesl<StandardResolver>,
}

impl ShaderLoader {
    // map "assets/shader/aaa.wesl" to this "package::aaa"
    fn handle_wesl_package_path(&self, path: &AssetPath) -> anyhow::Result<String> {
        let final_path = path.final_path();
        let relative = &final_path[self.root.len()..];
        let relative = relative.trim_start_matches(['/', '\\']);
        let path = Path::new(relative);
        let file_stem = path.with_extension("");
        let components: Vec<&str> = file_stem
            .components()
            .map(|c| c.as_os_str().to_str().unwrap())
            .collect();
        Ok(format!("package::{}", components.join("::")))
    }

    pub fn load_source(&mut self, path: AssetPath) -> anyhow::Result<wgpu::ShaderSource<'static>> {
        let source = self
            .wesl
            .compile(&self.handle_wesl_package_path(&path)?.parse()?)?
            .to_string();

        let shader_source = ShaderSource::Wgsl(Cow::Owned(source));

        Ok(shader_source)
    }

    pub fn load_module_by_world(
        world: &mut World,
        path: AssetPath,
    ) -> anyhow::Result<wgpu::ShaderModule> {
        let mut shader_loader = world.resource_mut::<ShaderLoader>();
        let shader_source = shader_loader.load_source(path)?;

        let rs = world.resource::<RenderState>();
        let device = &rs.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Prefiltering Env Map"),
            source: shader_source,
        });

        Ok(shader)
    }
}

impl FromWorld for ShaderLoader {
    fn from_world(_world: &mut World) -> Self {
        let root = "assets/shaders".to_string();
        let wesl = Wesl::new(&root);

        Self { root, wesl }
    }
}
