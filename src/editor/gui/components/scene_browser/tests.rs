use super::*;
use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput};

// Drive the actual egui controls; no mocked click flags or filesystem inventories.
struct BrowserUi {
    root: tempfile::TempDir,
    world: World,
    ctx: Context,
    time: f64,
    width: f32,
}
impl BrowserUi {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("scenes/nested")).unwrap();
        for name in ["Alpha", "beta", "gamma"] {
            std::fs::write(
                root.path().join(format!("scenes/{name}.scn.ron")),
                "browser does not parse scenes",
            )
            .unwrap();
        }
        let mut world = World::new();
        world.insert_resource(AssetRoot(root.path().into()));
        world.insert_resource(SceneController::default());
        world.insert_resource(lentille_render::asset::SceneAssetCache::default());
        world.init_resource::<SceneBrowser>();
        Self {
            root,
            world,
            ctx: Context::default(),
            time: 0.0,
            width: 900.0,
        }
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 0.08;
        self.ctx.run_ui(
            RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(self.width, 600.0),
                )),
                time: Some(self.time),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| show(ui, &mut self.world),
        )
    }
    fn settle(&mut self) -> egui::FullOutput {
        self.frame(vec![]);
        self.frame(vec![])
    }
    fn click(&mut self, pos: Pos2) -> egui::FullOutput {
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }])
    }
    fn search(&mut self, text: &str) -> egui::FullOutput {
        let output = self.settle();
        self.click(label(&output, "Search folders and scenes"));
        self.frame(vec![Event::Text(text.into())])
    }
    fn enter_scenes_from_tree(&mut self) -> egui::FullOutput {
        let output = self.settle();
        self.click(tree_label(&output, "scenes"));
        self.settle()
    }
}
fn positions(output: &egui::FullOutput, label: &str) -> Vec<Pos2> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .collect()
}
fn label(output: &egui::FullOutput, text: &str) -> Pos2 {
    *positions(output, text).first().unwrap_or_else(|| {
        panic!(
            "Missing visible browser label {text}; rendered text: {:?}",
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        )
    })
}
fn card_label(output: &egui::FullOutput, text: &str) -> Pos2 {
    positions(output, text)
        .into_iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("Missing card {text}"))
}
fn tree_label(output: &egui::FullOutput, text: &str) -> Pos2 {
    positions(output, text)
        .into_iter()
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap_or_else(|| panic!("Missing folder {text}"))
}

#[test]
fn cards_select_before_double_click_navigation_and_route_scene_open_unless_busy() {
    let mut ui = BrowserUi::new();
    let output = ui.settle();
    let folder = card_label(&output, "scenes");
    ui.click(folder);
    assert!(
        ui.world
            .resource::<SceneBrowser>()
            .directory
            .as_os_str()
            .is_empty()
    );
    assert_eq!(
        ui.world.resource::<SceneBrowser>().selected.as_deref(),
        Some(Path::new("scenes"))
    );
    ui.click(folder);
    let output = ui.settle();
    assert_eq!(
        ui.world.resource::<SceneBrowser>().directory,
        Path::new("scenes")
    );
    let scene = card_label(&output, "Alpha");
    // Start a separate gesture outside egui's global triple-click window.
    ui.time += 0.7;
    ui.click(scene);
    assert_eq!(
        ui.world.resource::<SceneBrowser>().selected.as_deref(),
        Some(Path::new("scenes/Alpha.scn.ron"))
    );
    assert!(ui.world.resource::<SceneController>().request.is_none());
    ui.click(scene);
    assert!(matches!(&ui.world.resource::<SceneController>().request,
        Some(SceneRequest::Open(path)) if path == Path::new("scenes/Alpha.scn.ron")));

    let mut controller = ui.world.resource_mut::<SceneController>();
    controller.request = None;
    controller.pending_open = Some("scenes/Alpha.scn.ron".into());
    let output = ui.settle();
    let beta = card_label(&output, "beta");
    ui.click(beta);
    ui.click(beta);
    assert!(
        ui.world.resource::<SceneController>().request.is_none(),
        "confirmation must block another open"
    );
    {
        let mut controller = ui.world.resource_mut::<SceneController>();
        controller.pending_open = None;
        controller.loading = Some(crate::scene::controller::SceneLoadProgress {
            id: 1,
            path: "scenes/Alpha.scn.ron".into(),
            stage: "Loading CPU".into(),
            detail: "Reading assets".into(),
            uploaded_bytes: 0,
            total_bytes: 0,
        });
    }
    let output = ui.settle();
    let folder = card_label(&output, "nested");
    ui.time += 0.7;
    ui.click(folder);
    ui.click(folder);
    assert_eq!(
        ui.world.resource::<SceneBrowser>().directory,
        Path::new("scenes/nested"),
        "Directory browsing remains available during loading"
    );
    let output = ui.settle();
    let cancel = positions(&output, "X")
        .into_iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    ui.click(cancel);
    assert!(matches!(
        ui.world.resource::<SceneController>().request,
        Some(SceneRequest::CancelLoad)
    ));
}

#[test]
fn tree_navigation_search_and_refresh_share_the_current_directory_without_loading_scenes() {
    let mut ui = BrowserUi::new();
    ui.world.resource_mut::<SceneController>().path = Some("scenes/beta.scn.ron".into());
    let output = ui.enter_scenes_from_tree();
    assert_eq!(
        ui.world.resource::<SceneBrowser>().directory,
        Path::new("scenes")
    );
    assert!(
        !positions(&output, "OPEN").is_empty(),
        "current scene is marked independently of selection"
    );
    let output = ui.search("aLpHa");
    assert!(!positions(&output, "Alpha").is_empty());
    assert!(positions(&output, "beta").is_empty());
    assert!(!positions(&output, "1 items").is_empty());
    ui.click(label(&output, "X"));
    let output = ui.settle();
    assert!(!positions(&output, "beta").is_empty());

    std::fs::write(ui.root.path().join("scenes/new.scn.ron"), "new scene").unwrap();
    std::fs::create_dir(ui.root.path().join("scenes/fresh")).unwrap();
    let output = ui.settle();
    assert!(
        positions(&output, "new").is_empty(),
        "external changes wait for refresh"
    );
    assert!(positions(&output, "fresh").is_empty());
    ui.click(label(&output, "Refresh"));
    let output = ui.settle();
    assert!(!positions(&output, "new").is_empty());
    assert_eq!(
        positions(&output, "fresh").len(),
        2,
        "refreshed tree and grid both contain the new folder"
    );
    let output = ui.search("does-not-exist");
    assert!(!positions(&output, "No matching folders or scenes").is_empty());
    ui.click(tree_label(&output, "nested"));
    let output = ui.settle();
    assert_eq!(
        ui.world.resource::<SceneBrowser>().directory,
        Path::new("scenes/nested")
    );
    assert!(!positions(&output, "No scenes or folders").is_empty());
    assert!(
        !positions(&output, "Search folders and scenes").is_empty(),
        "navigation clears directory-local search"
    );
    assert!(ui.world.resource::<SceneController>().request.is_none());
    assert!(!ui.world.resource::<SceneController>().dirty);
}

#[test]
fn grid_and_card_size_controls_reflow_without_losing_selection() {
    let mut ui = BrowserUi::new();
    let output = ui.enter_scenes_from_tree();
    let alpha = card_label(&output, "Alpha");
    let gamma = card_label(&output, "gamma");
    assert!((alpha.y - gamma.y).abs() < 1.0);
    ui.width = 500.0;
    let output = ui.settle();
    let alpha = card_label(&output, "Alpha");
    let gamma = card_label(&output, "gamma");
    assert!(gamma.y > alpha.y, "narrow grid wraps onto another row");
    ui.click(gamma);
    assert_eq!(
        ui.world.resource::<SceneBrowser>().selected.as_deref(),
        Some(Path::new("scenes/gamma.scn.ron"))
    );
    let output = ui.settle();
    ui.click(label(&output, "Sources"));
    let output = ui.settle();
    assert!(positions(&output, "SOURCES").is_empty());
    assert!(!positions(&output, "gamma").is_empty());
    assert_eq!(
        ui.world.resource::<SceneBrowser>().selected.as_deref(),
        Some(Path::new("scenes/gamma.scn.ron"))
    );
    assert!(ui.world.resource::<SceneController>().request.is_none());
    ui.click(label(&output, "View"));
    let output = ui.settle();
    let value = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().parse::<f32>().is_ok() => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("card size has an editable numeric value");
    ui.time += 0.7;
    ui.click(value);
    ui.click(value);
    ui.frame(vec![
        Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
        },
        Event::Text("144".into()),
        Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
    ]);
    assert_eq!(ui.world.resource::<SceneBrowser>().card_size, 144.0);
    let output = ui.settle();
    assert!(
        card_label(&output, "gamma").y > card_label(&output, "Alpha").y,
        "larger cards reduce the column count"
    );
}
