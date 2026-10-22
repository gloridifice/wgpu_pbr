use super::AssetPath;
use anyhow::{Context, Result, bail};
use bevy_ecs::prelude::Resource;
use bevy_reflect::Reflect;
use std::{
    marker::PhantomData,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub enum SubAsset {
    File,
    Mesh(usize),
    Material(usize),
    Image(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct MeshAsset;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct TextureAsset;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct AssetRef<T: bevy_reflect::TypePath + Send + Sync + 'static> {
    pub path: AssetPath,
    pub label: SubAsset,
    #[reflect(ignore)]
    marker: PhantomData<T>,
}

impl<T: bevy_reflect::TypePath + Send + Sync + 'static> AssetRef<T> {
    pub fn new(path: &str, label: SubAsset) -> Self {
        Self {
            path: AssetPath::new(path),
            label,
            marker: PhantomData,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct AssetRoot(pub PathBuf);
impl Default for AssetRoot {
    fn default() -> Self {
        Self(PathBuf::from("assets"))
    }
}

pub fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

impl AssetRoot {
    pub fn relative(path: &Path) -> Result<PathBuf> {
        let raw = path.to_str().context("Asset path is not UTF-8")?;
        if raw.contains(':') {
            bail!("Asset path must be relative: {raw}");
        }
        let normalized = raw.replace('\\', "/");
        let mut clean = PathBuf::new();
        for part in Path::new(&normalized).components() {
            match part {
                Component::Normal(p) => clean.push(p),
                Component::CurDir => (),
                _ => bail!("Asset path escapes root: {}", path.display()),
            }
        }
        Ok(clean)
    }

    pub fn resolve_path(&self, relative: &Path) -> Result<PathBuf> {
        let relative = Self::relative(relative)?;
        anyhow::ensure!(
            !is_link(&std::fs::symlink_metadata(&self.0)?),
            "Asset root cannot be a link"
        );
        let root = self
            .0
            .canonicalize()
            .with_context(|| format!("Asset root {}", self.0.display()))?;
        let mut result = root.clone();
        for component in relative.components() {
            result.push(component.as_os_str());
            let metadata = std::fs::symlink_metadata(&result)
                .with_context(|| format!("Asset {}", result.display()))?;
            anyhow::ensure!(
                !is_link(&metadata),
                "Asset links are not supported: {}",
                result.display()
            );
        }
        let canonical = result.canonicalize()?;
        anyhow::ensure!(
            canonical.starts_with(&root),
            "Asset outside root: {}",
            result.display()
        );
        Ok(canonical)
    }

    pub fn resolve(&self, path: &AssetPath) -> Result<PathBuf> {
        let AssetPath::Assets(path) = path;
        self.resolve_path(Path::new(path))
    }

    pub fn save_path(&self, path: &Path) -> Result<PathBuf> {
        let relative = Self::relative(path)?;
        let name = relative.file_name().context("Missing file name")?;
        let parent = self.resolve_path(relative.parent().unwrap_or(Path::new("")))?;
        anyhow::ensure!(parent.is_dir(), "Save parent is not a directory");
        let result = parent.join(name);
        if result.exists() {
            anyhow::ensure!(
                result.is_file() && !is_link(&std::fs::symlink_metadata(&result)?),
                "Save target is not a regular file"
            );
        }
        Ok(result)
    }
}
