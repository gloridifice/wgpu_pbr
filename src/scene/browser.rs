use super::codec::is_scene_file;
use anyhow::{Context, Result, ensure};
use lentille_render::asset::{AssetRoot, path::is_link};
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserEntryKind {
    Folder,
    Scene,
}
#[derive(Clone, Debug)]
pub struct BrowserEntry {
    pub path: PathBuf,
    pub display_name: String,
    pub kind: BrowserEntryKind,
}
pub fn scan_scene_directory(root: &AssetRoot, directory: &Path) -> Result<Vec<BrowserEntry>> {
    let path = root.resolve_path(directory)?;
    ensure!(path.is_dir(), "Not a directory: {}", path.display());
    let mut entries = Vec::new();
    for entry in
        std::fs::read_dir(&path).with_context(|| format!("Read directory {}", path.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let display_name = name.to_string_lossy().into_owned();
        if display_name.starts_with('.') {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if is_link(&metadata) {
            continue;
        }
        let kind = if metadata.is_dir() {
            BrowserEntryKind::Folder
        } else if metadata.is_file() && is_scene_file(&entry.path()) {
            BrowserEntryKind::Scene
        } else {
            continue;
        };
        entries.push(BrowserEntry {
            path: directory.join(name),
            display_name,
            kind,
        });
    }
    entries.sort_by_key(|e| {
        (
            if e.kind == BrowserEntryKind::Folder {
                0
            } else {
                1
            },
            e.display_name.to_lowercase(),
            e.display_name.clone(),
        )
    });
    Ok(entries)
}
