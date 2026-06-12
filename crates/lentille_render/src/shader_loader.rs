use std::{
    borrow::Cow,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use bevy_ecs::prelude::*;
use wesl::{Resolver, StandardResolver, Wesl, syntax::ModulePath};
use wgpu::ShaderSource;

use crate::{RenderState, asset::AssetPath};

/// Bump when the compiled-WGSL format or compile options change so every cached
/// artifact is invalidated. Tied to the `wesl` crate version.
const CACHE_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "-wesl-0.4.0");

#[derive(Resource)]
pub struct ShaderLoader {
    pub root: String,
    pub wesl: Wesl<StandardResolver>,
    cache_dir: Option<PathBuf>,
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
        let entry = self.handle_wesl_package_path(&path)?;

        if let Some(source) = self.try_load_cache(&entry) {
            return Ok(ShaderSource::Wgsl(Cow::Owned(source)));
        }

        let module_path: ModulePath = entry.parse()?;
        let result = self.wesl.compile(&module_path)?;
        let source = result.to_string();

        self.store_cache(&entry, &result.modules, &source);

        Ok(ShaderSource::Wgsl(Cow::Owned(source)))
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

    /// File-name prefix for the cache entry, derived from the module path and the
    /// cache version. The dependency timestamps live inside the `.meta` file.
    fn cache_key(entry: &str) -> String {
        let mut hasher = DefaultHasher::new();
        entry.hash(&mut hasher);
        CACHE_VERSION.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    fn cache_paths(&self, entry: &str) -> Option<(PathBuf, PathBuf)> {
        let dir = self.cache_dir.as_ref()?;
        let key = Self::cache_key(entry);
        Some((dir.join(format!("{key}.wgsl")), dir.join(format!("{key}.meta"))))
    }

    /// Return the compiled WGSL from disk when the cache is present and every
    /// source file it depends on is unchanged.
    fn try_load_cache(&self, entry: &str) -> Option<String> {
        let (wgsl_path, meta_path) = self.cache_paths(entry)?;
        let meta = std::fs::read_to_string(&meta_path).ok()?;

        let mut lines = meta.lines();
        if lines.next()? != CACHE_VERSION {
            return None;
        }

        for line in lines {
            let (mtime_str, dep_path) = line.split_once('\t')?;
            let stored: u64 = mtime_str.parse().ok()?;
            let current = file_mtime(Path::new(dep_path))?;
            if current != stored {
                return None;
            }
        }

        std::fs::read_to_string(&wgsl_path).ok()
    }

    /// Persist the compiled WGSL plus the timestamps of every source module that
    /// took part in the compilation. Failures are non-fatal.
    fn store_cache(&self, entry: &str, modules: &[ModulePath], source: &str) {
        let Some((wgsl_path, meta_path)) = self.cache_paths(entry) else {
            return;
        };

        let mut meta = String::new();
        meta.push_str(CACHE_VERSION);
        meta.push('\n');

        for module in modules {
            let Some(fs_path) = self.wesl.resolver().fs_path(module) else {
                continue;
            };
            let Some(mtime) = file_mtime(&fs_path) else {
                continue;
            };
            meta.push_str(&format!("{mtime}\t{}\n", fs_path.display()));
        }

        if std::fs::write(&wgsl_path, source).is_ok() {
            let _ = std::fs::write(&meta_path, meta);
        }
    }
}

fn file_mtime(path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

impl FromWorld for ShaderLoader {
    fn from_world(_world: &mut World) -> Self {
        let root = "assets/shaders".to_string();
        let wesl = Wesl::new(&root);

        let cache_dir = if std::env::var_os("WGPU_PBR_NO_SHADER_CACHE").is_some() {
            None
        } else {
            let dir = PathBuf::from(".cache/shaders");
            match std::fs::create_dir_all(&dir) {
                Ok(()) => Some(dir),
                Err(_) => None,
            }
        };

        Self {
            root,
            wesl,
            cache_dir,
        }
    }
}
