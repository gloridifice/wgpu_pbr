use std::{borrow::Cow, fs};

use bevy_ecs::prelude::*;
use naga_oil::compose::Composer;
use wgpu::ShaderSource;

use crate::asset::AssetPath;

#[derive(Resource)]
pub struct ShaderLoader {
    pub composer: Composer,
}

impl ShaderLoader {
    pub fn load_source(&mut self, path: AssetPath) -> anyhow::Result<wgpu::ShaderSource<'static>> {
        let final_path = path.final_path();
        let string = match fs::read_to_string(&final_path) {
            Ok(s) => s,
            Err(e) => {
                panic!("Load Shader Failed: {} \n Err: {}", &final_path, e)
            }
        };
        let source = self
            .composer
            .make_naga_module(naga_oil::compose::NagaModuleDescriptor {
                source: &string,
                file_path: &final_path,
                ..Default::default()
            })?;
        Ok(ShaderSource::Naga(Cow::Owned(source)))
    }
}

impl FromWorld for ShaderLoader {
    fn from_world(_world: &mut World) -> Self {
        let mut composer = Composer::default();
        let paths =
            fs::read_dir(AssetPath::Assets("shaders/libs/".to_string()).final_path()).unwrap();
        for path in paths {
            let path = &path.unwrap().path();
            let shader_string = fs::read_to_string(path).unwrap();
            match composer.add_composable_module(naga_oil::compose::ComposableModuleDescriptor {
                source: &shader_string,
                file_path: path.to_str().unwrap(),
                ..Default::default()
            }) {
                Ok(_) => {}
                Err(e) => println!("? -> {e:#?}"),
            }
        }
        Self { composer }
    }
}
