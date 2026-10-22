use crate::scene::{
    SceneController, SceneRequest,
    browser::{BrowserEntry, BrowserEntryKind, scan_scene_directory},
    codec::is_scene_file,
};
use bevy_ecs::prelude::*;
use lentille_render::asset::AssetRoot;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

mod widgets;

#[derive(Default)]
struct DirectoryListing {
    entries: Vec<BrowserEntry>,
    error: Option<String>,
}

#[derive(Resource)]
pub struct SceneBrowser {
    directory: PathBuf,
    listings: HashMap<PathBuf, DirectoryListing>,
    selected: Option<PathBuf>,
    error: Option<String>,
    loaded: bool,
    refresh: u64,
    search: String,
    card_size: f32,
    sources_visible: bool,
    reveal_directory: bool,
    path_input: String,
    overwrite: Option<PathBuf>,
    open_dialog: bool,
}
impl Default for SceneBrowser {
    fn default() -> Self {
        Self {
            directory: PathBuf::new(),
            listings: HashMap::new(),
            selected: None,
            error: None,
            loaded: false,
            refresh: 0,
            search: String::new(),
            card_size: 96.0,
            sources_visible: true,
            reveal_directory: true,
            path_input: String::new(),
            overwrite: None,
            open_dialog: false,
        }
    }
}
impl SceneBrowser {
    fn listing(&mut self, root: &AssetRoot, directory: &Path) -> &DirectoryListing {
        self.listings
            .entry(directory.to_path_buf())
            .or_insert_with(|| match scan_scene_directory(root, directory) {
                Ok(entries) => DirectoryListing {
                    entries,
                    error: None,
                },
                Err(error) => DirectoryListing {
                    entries: Vec::new(),
                    error: Some(format!("{error:#}")),
                },
            })
    }
    fn refresh(&mut self, root: &AssetRoot) {
        if !self.directory.as_os_str().is_empty()
            && !root
                .resolve_path(&self.directory)
                .is_ok_and(|path| path.is_dir())
        {
            self.directory.clear();
            self.selected = None;
            self.search.clear();
            self.reveal_directory = true;
            self.error = Some("Directory unavailable; returned to assets".into());
        }
        let directory = self.directory.clone();
        self.listings.remove(&directory);
        let listing = self.listing(root, &directory);
        let error = listing.error.clone();
        let selected_exists = self.selected.as_ref().is_none_or(|selected| {
            self.listings[&directory]
                .entries
                .iter()
                .any(|entry| &entry.path == selected)
        });
        if !selected_exists {
            self.selected = None;
        }
        if error.is_some() {
            self.error = error;
        }
        self.loaded = true;
    }
    fn navigate(&mut self, root: &AssetRoot, directory: PathBuf) {
        match root.resolve_path(&directory) {
            Ok(path) if path.is_dir() => {
                self.directory = directory;
                self.selected = None;
                self.search.clear();
                self.error = None;
                self.loaded = false;
                self.reveal_directory = true;
            }
            Ok(_) => self.error = Some("Folder unavailable; refresh the directory".into()),
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }
}

pub fn show(ui: &mut egui::Ui, world: &mut World) {
    let root = world.resource::<AssetRoot>().clone();
    world.resource_scope(|world, mut browser: Mut<SceneBrowser>| {
        let controller = world.resource::<SceneController>();
        if browser.refresh != controller.refresh {
            browser.refresh = controller.refresh;
            browser.listings.clear();
            browser.loaded = false;
            browser.error = None;
        }
        if !browser.loaded {
            browser.refresh(&root);
        }
        let busy = controller.request.is_some()
            || controller.pending_open.is_some()
            || controller.needs_save_path;
        let mut navigate = None;
        let mut activated = None;
        ui.scope(|ui| {
            widgets::style(ui);
            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
            egui::Panel::top(ui.make_persistent_id("asset_toolbar"))
                .frame(egui::Frame::new().fill(widgets::SURFACE).inner_margin(6))
                .show_inside(ui, |ui| {
                    toolbar(ui, &mut browser, &mut navigate);
                    let errors = [controller.error.as_ref(), browser.error.as_ref()];
                    for error in errors.into_iter().flatten() {
                        let text = egui::RichText::new(error).color(egui::Color32::LIGHT_RED);
                        ui.add(egui::Label::new(text).truncate())
                            .on_hover_text(error);
                    }
                });
            if !browser.loaded {
                browser.refresh(&root);
            }
            let directory = browser.directory.clone();
            let search = browser.search.trim().to_lowercase();
            let entries: Vec<_> = browser
                .listing(&root, &directory)
                .entries
                .iter()
                .filter(|entry| entry.display_name.to_lowercase().contains(&search))
                .cloned()
                .collect();
            egui::Panel::bottom(ui.make_persistent_id("asset_footer"))
                .frame(
                    egui::Frame::new()
                        .fill(widgets::SURFACE)
                        .inner_margin(egui::Margin::symmetric(8, 4)),
                )
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        let menu = egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
                        egui::containers::menu::MenuButton::new("View")
                            .config(menu)
                            .ui(ui, |ui| {
                                ui.label("Card size");
                                ui.add(
                                    egui::Slider::new(&mut browser.card_size, 64.0..=144.0)
                                        .suffix(" px"),
                                );
                                ui.checkbox(&mut browser.sources_visible, "Show Sources");
                            });
                        ui.weak(format!("{} items", entries.len()));
                        let current = controller
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "Untitled".into());
                        let counts = world
                            .resource::<lentille_render::asset::SceneAssetCache>()
                            .counts();
                        let label = format!(
                            "Current: {current}{}",
                            if controller.dirty { " *" } else { "" }
                        );
                        let tooltip = format!(
                            "{}\n{current}\nCached: {} models / {} meshes / {} textures",
                            controller.status, counts.0, counts.1, counts.2
                        );
                        let width = (ui.available_width() - if busy { 24.0 } else { 0.0 }).max(1.0);
                        ui.add_sized([width, 22.0], egui::Label::new(label).truncate())
                            .on_hover_text(tooltip);
                        if busy {
                            ui.spinner();
                        }
                    });
                });
            if browser.sources_visible && ui.available_width() >= 320.0 {
                egui::Panel::left(ui.make_persistent_id("asset_sources"))
                    .default_size((ui.available_width() * 0.30).clamp(120.0, 200.0))
                    .min_size(110.0)
                    .max_size((ui.available_width() - 140.0).max(110.0))
                    .frame(egui::Frame::new().fill(widgets::BACKGROUND).inner_margin(4))
                    .show_inside(ui, |ui| {
                        ui.label(
                            egui::RichText::new("SOURCES")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        egui::ScrollArea::vertical()
                            .id_salt("asset_folder_tree")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                folder_tree(
                                    ui,
                                    &root,
                                    &mut browser,
                                    Path::new(""),
                                    "assets",
                                    &mut navigate,
                                );
                            });
                    });
                browser.reveal_directory = false;
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(widgets::BACKGROUND).inner_margin(8))
                .show_inside(ui, |ui| {
                    activated = grid(ui, &mut browser, &entries, controller.path.as_deref(), busy);
                });
        });
        if let Some(entry) = activated {
            match root.resolve_path(&entry.path) {
                Ok(path) if entry.kind == BrowserEntryKind::Folder && path.is_dir() => {
                    navigate = Some(entry.path);
                }
                Ok(path) if entry.kind == BrowserEntryKind::Scene && path.is_file() => {
                    world
                        .resource_mut::<SceneController>()
                        .request(SceneRequest::Open(entry.path));
                }
                Ok(_) => browser.error = Some("Entry changed; refresh the directory".into()),
                Err(error) => browser.error = Some(format!("{error:#}")),
            }
        }
        if let Some(directory) = navigate {
            browser.navigate(&root, directory);
        }
    });
}

fn grid(
    ui: &mut egui::Ui,
    browser: &mut SceneBrowser,
    entries: &[BrowserEntry],
    current: Option<&Path>,
    busy: bool,
) -> Option<BrowserEntry> {
    let mut activated = None;
    ui.add_enabled_ui(!busy, |ui| {
        if entries.is_empty() && browser.error.is_none() {
            ui.weak(if browser.search.trim().is_empty() {
                "No scenes or folders"
            } else {
                "No matching folders or scenes"
            });
        }
        // Reserve scrollbar space so its appearance does not change column count.
        let available = (ui.available_width() - 20.0).max(32.0);
        let width = browser.card_size.min(available);
        let spacing = ui.spacing().item_spacing.x;
        let columns = ((available + spacing) / (width + spacing)).floor().max(1.0) as usize;
        let rows = entries.len().div_ceil(columns);
        egui::ScrollArea::vertical()
            .id_salt((
                "asset_cards",
                &browser.directory,
                browser.search.trim().to_lowercase(),
            ))
            .auto_shrink([false, false])
            .show_rows(ui, width + 42.0, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        for entry in
                            &entries[row * columns..((row + 1) * columns).min(entries.len())]
                        {
                            let response = ui
                                .push_id(&entry.path, |ui| {
                                    widgets::card(
                                        ui,
                                        entry,
                                        width,
                                        browser.selected.as_ref() == Some(&entry.path),
                                        current == Some(entry.path.as_path()),
                                    )
                                })
                                .inner;
                            if response.clicked() {
                                browser.selected = Some(entry.path.clone());
                            }
                            if response.double_clicked() {
                                activated = Some(entry.clone());
                            }
                        }
                    });
                }
            });
    });
    activated
}

fn toolbar(ui: &mut egui::Ui, browser: &mut SceneBrowser, navigate: &mut Option<PathBuf>) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(browser.sources_visible, "Sources")
            .on_hover_text("Toggle folder tree (hidden automatically in narrow panels)")
            .clicked()
        {
            browser.sources_visible = !browser.sources_visible;
        }
        if ui
            .add_enabled(
                !browser.directory.as_os_str().is_empty(),
                egui::Button::new("Up"),
            )
            .on_hover_text("Parent folder")
            .clicked()
        {
            *navigate = Some(
                browser
                    .directory
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_path_buf(),
            );
        }
        if ui.button("Refresh").clicked() {
            browser.listings.clear();
            browser.error = None;
            browser.loaded = false;
        }
        ui.separator();
        egui::ScrollArea::horizontal()
            .id_salt("asset_breadcrumbs")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.link("assets").on_hover_text("Assets root").clicked() {
                        *navigate = Some(PathBuf::new());
                    }
                    let mut path = PathBuf::new();
                    for component in browser.directory.components() {
                        path.push(component.as_os_str());
                        ui.weak("/");
                        if ui.link(component.as_os_str().to_string_lossy()).clicked() {
                            *navigate = Some(path.clone());
                        }
                    }
                });
            });
    });
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("SCENES")
                .small()
                .color(egui::Color32::from_rgb(100, 170, 235)),
        );
        let width = (ui.available_width() - 34.0).max(24.0);
        ui.add_sized(
            [width, 22.0],
            egui::TextEdit::singleline(&mut browser.search)
                .id_salt("asset_search")
                .hint_text("Search folders and scenes"),
        );
        if ui
            .add_enabled(!browser.search.is_empty(), egui::Button::new("X"))
            .on_hover_text("Clear search")
            .clicked()
        {
            browser.search.clear();
        }
    });
}

fn folder_tree(
    ui: &mut egui::Ui,
    root: &AssetRoot,
    browser: &mut SceneBrowser,
    directory: &Path,
    label: &str,
    navigate: &mut Option<PathBuf>,
) {
    let id = ui.make_persistent_id(("asset_folder", directory));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        directory.as_os_str().is_empty(),
    );
    if browser.reveal_directory && browser.directory.starts_with(directory) {
        state.set_open(true);
    }
    state
        .show_header(ui, |ui| {
            if widgets::folder_row(ui, label, browser.directory == directory)
                .on_hover_text(format!("assets/{}", directory.display()))
                .clicked()
            {
                *navigate = Some(directory.to_path_buf());
            }
        })
        .body(|ui| {
            let listing = browser.listing(root, directory);
            let folders: Vec<_> = listing
                .entries
                .iter()
                .filter(|entry| entry.kind == BrowserEntryKind::Folder)
                .cloned()
                .collect();
            if let Some(error) = &listing.error {
                ui.colored_label(egui::Color32::LIGHT_RED, "Folder unavailable")
                    .on_hover_text(error);
            }
            for entry in folders {
                folder_tree(
                    ui,
                    root,
                    browser,
                    &entry.path,
                    &entry.display_name,
                    navigate,
                );
            }
        });
}
pub fn file_menu(ui: &mut egui::Ui, world: &mut World) {
    if ui.button("Open…").clicked() {
        world.resource_mut::<SceneBrowser>().open_dialog = true;
        ui.close();
    }
    if ui.button("Save").clicked() {
        world
            .resource_mut::<SceneController>()
            .request(SceneRequest::Save);
        ui.close();
    }
    if ui.button("Save As…").clicked() {
        world.resource_mut::<SceneController>().needs_save_path = true;
        ui.close();
    }
    if ui.button("Reload").clicked() {
        world
            .resource_mut::<SceneController>()
            .request(SceneRequest::Reload);
        ui.close();
    }
}
pub fn dialogs(ctx: &egui::Context, world: &mut World) {
    let root = world.resource::<AssetRoot>().clone();
    world.resource_scope(|world, mut browser: Mut<SceneBrowser>| {
        let mut controller = world.resource_mut::<SceneController>();
        if controller.pending_open.is_some() && !controller.save_then_open {
            egui::Modal::new(egui::Id::new("scene_unsaved")).show(ctx, |ui| {
                ui.heading("Unsaved Scene");
                ui.label("Save changes before opening another scene?");
                ui.horizontal(|ui| {
                    if ui.button("Save and Open").clicked() {
                        controller.resolve_unsaved(crate::scene::controller::UnsavedDecision::Save);
                    }
                    if ui.button("Discard and Open").clicked() {
                        controller
                            .resolve_unsaved(crate::scene::controller::UnsavedDecision::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        controller
                            .resolve_unsaved(crate::scene::controller::UnsavedDecision::Cancel);
                    }
                });
            });
        }
        if controller.needs_save_path || browser.open_dialog {
            if browser.path_input.is_empty() {
                browser.path_input = "scenes/untitled.scn.ron".into();
            }
            let opening = browser.open_dialog;
            egui::Modal::new(egui::Id::new("scene_path")).show(ctx, |ui| {
                ui.heading(if opening {
                    "Open Scene"
                } else {
                    "Save Scene As"
                });
                ui.horizontal(|ui| {
                    ui.label("assets/");
                    ui.text_edit_singleline(&mut browser.path_input);
                });
                if let Some(error) = &controller.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui.button(if opening { "Open" } else { "Save" }).clicked() {
                        let mut path = PathBuf::from(browser.path_input.trim());
                        if !is_scene_file(&path) && path.extension().is_none() {
                            path.set_extension("scn.ron");
                        }
                        if opening {
                            controller.request = Some(SceneRequest::Open(path));
                            browser.open_dialog = false;
                        } else {
                            match world_path(&root, &path) {
                                Ok(true) => browser.overwrite = Some(path),
                                Ok(false) => {
                                    controller.request = Some(SceneRequest::SaveAs(path));
                                    controller.needs_save_path = false;
                                }
                                Err(error) => controller.error = Some(format!("{error:#}")),
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        controller.needs_save_path = false;
                        controller.save_then_open = false;
                        controller.pending_open = None;
                        browser.open_dialog = false;
                    }
                });
            });
        }
        if let Some(path) = browser.overwrite.clone() {
            egui::Modal::new(egui::Id::new("scene_overwrite")).show(ctx, |ui| {
                ui.heading("Replace Scene?");
                ui.label(path.display().to_string());
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        controller.request = Some(SceneRequest::SaveAs(path));
                        controller.needs_save_path = false;
                        browser.overwrite = None;
                    }
                    if ui.button("Cancel").clicked() {
                        browser.overwrite = None;
                    }
                });
            });
        }
    });
}
fn world_path(root: &AssetRoot, path: &std::path::Path) -> anyhow::Result<bool> {
    anyhow::ensure!(is_scene_file(path), "Scene file must end with .scn.ron");
    Ok(root.save_path(path)?.exists())
}

#[cfg(test)]
mod tests;
