use egui::{style::WidgetVisuals, *};
use egui_dock::{
    ButtonsStyle, LeafHighlighting, OverlayFeel, OverlayStyle, OverlayType, SeparatorStyle,
    Style as DockStyle, TabAddAlign, TabBarStyle, TabBodyStyle, TabInteractionStyle, TabStyle,
};

// ===== Helper widgets =====

fn corner_radius_ui(ui: &mut Ui, cr: &mut CornerRadius, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        if cr.is_same() {
            let mut v = cr.nw;
            if ui
                .add(DragValue::new(&mut v).range(0..=255).speed(0.5).prefix("all: "))
                .changed()
            {
                *cr = CornerRadius::same(v);
            }
        } else {
            for (name, field) in [("NW", &mut cr.nw), ("NE", &mut cr.ne), ("SW", &mut cr.sw), ("SE", &mut cr.se)] {
                ui.label(name);
                let mut v = *field;
                if ui.add(DragValue::new(&mut v).range(0..=255).speed(0.5)).changed() {
                    *field = v;
                }
            }
        }
    });
}

fn margin_ui(ui: &mut Ui, m: &mut Margin, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        for (name, field) in [("L", &mut m.left), ("R", &mut m.right), ("T", &mut m.top), ("B", &mut m.bottom)] {
            ui.label(name);
            let mut v = *field as f32;
            if ui.add(DragValue::new(&mut v).range(-128.0..=127.0).speed(0.5)).changed() {
                *field = v.round() as i8;
            }
        }
    });
}

fn option_margin_ui(ui: &mut Ui, label: &str, opt: &mut Option<Margin>, default_val: Margin) {
    ui.horizontal(|ui| {
        let mut enabled = opt.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            if enabled { *opt = Some(default_val); } else { *opt = None; }
        }
        if let Some(m) = opt.as_mut() { margin_ui(ui, m, label); } else { ui.label(label); }
    });
}

fn stroke_ui(ui: &mut Ui, s: &mut Stroke, label: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label("W:");
        ui.add(DragValue::new(&mut s.width).range(0.0..=20.0).speed(0.1));
        ui.label("C:");
        ui.color_edit_button_srgba(&mut s.color);
    });
}

fn color_row(ui: &mut Ui, label: &str, c: &mut Color32) {
    ui.horizontal(|ui| { ui.label(label); ui.color_edit_button_srgba(c); });
}

fn f32_row(ui: &mut Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, speed: f32) {
    ui.horizontal(|ui| { ui.label(label); ui.add(DragValue::new(v).range(range).speed(speed)); });
}

fn option_f32_row(ui: &mut Ui, label: &str, opt: &mut Option<f32>, default_val: f32) {
    ui.horizontal(|ui| {
        let mut enabled = opt.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            if enabled { *opt = Some(default_val); } else { *opt = None; }
        }
        if let Some(v) = opt.as_mut() { ui.add(DragValue::new(v).speed(0.5)); } else { ui.label(label); }
    });
}

fn vec2_ui(ui: &mut Ui, label: &str, v: &mut Vec2, range: std::ops::RangeInclusive<f32>, speed: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label("x:");
        ui.add(DragValue::new(&mut v.x).range(range.clone()).speed(speed));
        ui.label("y:");
        ui.add(DragValue::new(&mut v.y).range(range).speed(speed));
    });
}

// ===== Egui native style editors =====

fn widget_visuals_ui(ui: &mut Ui, name: &str, wv: &mut WidgetVisuals) {
    CollapsingHeader::new(name).default_open(false).show(ui, |ui| {
        color_row(ui, "Bg Fill", &mut wv.bg_fill);
        stroke_ui(ui, &mut wv.bg_stroke, "Bg Stroke");
        stroke_ui(ui, &mut wv.fg_stroke, "Fg Stroke");
        corner_radius_ui(ui, &mut wv.corner_radius, "Corner Radius");
        f32_row(ui, "Expansion", &mut wv.expansion, 0.0..=50.0, 0.5);
    });
}

fn spacing_ui(ui: &mut Ui, sp: &mut Spacing) {
    vec2_ui(ui, "Item Spacing", &mut sp.item_spacing, 0.0..=40.0, 0.5);
    margin_ui(ui, &mut sp.window_margin, "Window Margin");
    vec2_ui(ui, "Button Padding", &mut sp.button_padding, 0.0..=40.0, 0.5);
    f32_row(ui, "Indent", &mut sp.indent, 0.0..=100.0, 1.0);
    vec2_ui(ui, "Interact Size", &mut sp.interact_size, 0.0..=80.0, 1.0);
    f32_row(ui, "Slider Width", &mut sp.slider_width, 20.0..=300.0, 1.0);
}

fn egui_style_ui(ui: &mut Ui, style: &mut egui::Style) {
    // --- Visuals ---
    CollapsingHeader::new("🌙 Visuals").default_open(true).show(ui, |ui| {
        let v = &mut style.visuals;
        ui.checkbox(&mut v.dark_mode, "Dark Mode");
        color_row(ui, "Window Fill", &mut v.window_fill);
        color_row(ui, "Panel Fill", &mut v.panel_fill);
        color_row(ui, "Extreme Bg", &mut v.extreme_bg_color);
        color_row(ui, "Faint Bg", &mut v.faint_bg_color);
        color_row(ui, "Code Bg", &mut v.code_bg_color);
        corner_radius_ui(ui, &mut v.window_corner_radius, "Window Corner Radius");
        stroke_ui(ui, &mut v.window_stroke, "Window Stroke");
        color_row(ui, "Hyperlink", &mut v.hyperlink_color);
        color_row(ui, "Warn Fg", &mut v.warn_fg_color);
        color_row(ui, "Error Fg", &mut v.error_fg_color);

        // Override text color (optional)
        ui.horizontal(|ui| {
            let mut has_override = v.override_text_color.is_some();
            if ui.checkbox(&mut has_override, "Text Color Override").changed() {
                if has_override { v.override_text_color = Some(Color32::WHITE); }
                else { v.override_text_color = None; }
            }
        });
        if let Some(ref mut tc) = v.override_text_color {
            ui.horizontal(|ui| { ui.label("  "); ui.color_edit_button_srgba(tc); });
        }
    });

    // --- Widget Visuals ---
    CollapsingHeader::new("🔲 Widgets").default_open(false).show(ui, |ui| {
        widget_visuals_ui(ui, "Noninteractive", &mut style.visuals.widgets.noninteractive);
        widget_visuals_ui(ui, "Inactive", &mut style.visuals.widgets.inactive);
        widget_visuals_ui(ui, "Hovered", &mut style.visuals.widgets.hovered);
        widget_visuals_ui(ui, "Active", &mut style.visuals.widgets.active);
        widget_visuals_ui(ui, "Open", &mut style.visuals.widgets.open);
    });

    // --- Spacing ---
    CollapsingHeader::new("📏 Spacing").default_open(false).show(ui, |ui| {
        spacing_ui(ui, &mut style.spacing);
    });
}

// ===== Dock style editors (unchanged from before) =====

fn tab_interaction_style_ui(ui: &mut Ui, name: &str, s: &mut TabInteractionStyle) {
    CollapsingHeader::new(name).default_open(false).show(ui, |ui| {
        color_row(ui, "Outline", &mut s.outline_color);
        corner_radius_ui(ui, &mut s.corner_radius, "Radius");
        color_row(ui, "Bg Fill", &mut s.bg_fill);
        color_row(ui, "Text", &mut s.text_color);
    });
}

fn tab_body_style_ui(ui: &mut Ui, s: &mut TabBodyStyle) {
    margin_ui(ui, &mut s.inner_margin, "Inner Margin");
    stroke_ui(ui, &mut s.stroke, "Stroke");
    corner_radius_ui(ui, &mut s.corner_radius, "Radius");
    color_row(ui, "Bg Fill", &mut s.bg_fill);
}

fn buttons_style_ui(ui: &mut Ui, s: &mut ButtonsStyle) {
    CollapsingHeader::new("Add Tab Button").default_open(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Align:");
            ui.selectable_value(&mut s.add_tab_align, TabAddAlign::Left, "Left");
            ui.selectable_value(&mut s.add_tab_align, TabAddAlign::Right, "Right");
        });
        color_row(ui, "Color", &mut s.add_tab_color);
        color_row(ui, "Active Color", &mut s.add_tab_active_color);
        color_row(ui, "Bg Fill", &mut s.add_tab_bg_fill);
        color_row(ui, "Border Color", &mut s.add_tab_border_color);
    });
    CollapsingHeader::new("Close Tab Button").default_open(false).show(ui, |ui| {
        color_row(ui, "Color", &mut s.close_tab_color);
        color_row(ui, "Active Color", &mut s.close_tab_active_color);
        color_row(ui, "Bg Fill", &mut s.close_tab_bg_fill);
    });
    CollapsingHeader::new("Close All Button").default_open(false).show(ui, |ui| {
        color_row(ui, "Color", &mut s.close_all_tabs_color);
        color_row(ui, "Active Color", &mut s.close_all_tabs_active_color);
        color_row(ui, "Bg Fill", &mut s.close_all_tabs_bg_fill);
        color_row(ui, "Border Color", &mut s.close_all_tabs_border_color);
        color_row(ui, "Disabled Color", &mut s.close_all_tabs_disabled_color);
    });
    CollapsingHeader::new("Collapse Button").default_open(false).show(ui, |ui| {
        color_row(ui, "Color", &mut s.collapse_tabs_color);
        color_row(ui, "Active Color", &mut s.collapse_tabs_active_color);
        color_row(ui, "Bg Fill", &mut s.collapse_tabs_bg_fill);
        color_row(ui, "Border Color", &mut s.collapse_tabs_border_color);
    });
    CollapsingHeader::new("Minimize Button").default_open(false).show(ui, |ui| {
        color_row(ui, "Color", &mut s.minimize_window_color);
        color_row(ui, "Active Color", &mut s.minimize_window_active_color);
        color_row(ui, "Bg Fill", &mut s.minimize_window_bg_fill);
        color_row(ui, "Border Color", &mut s.minimize_window_border_color);
    });
}

fn separator_style_ui(ui: &mut Ui, s: &mut SeparatorStyle) {
    f32_row(ui, "Width", &mut s.width, 0.0..=20.0, 0.1);
    f32_row(ui, "Extra Interact", &mut s.extra_interact_width, 0.0..=50.0, 0.5);
    f32_row(ui, "Extra", &mut s.extra, 0.0..=500.0, 1.0);
    color_row(ui, "Idle", &mut s.color_idle);
    color_row(ui, "Hovered", &mut s.color_hovered);
    color_row(ui, "Dragged", &mut s.color_dragged);
}

fn tab_bar_style_ui(ui: &mut Ui, s: &mut TabBarStyle) {
    color_row(ui, "Bg Fill", &mut s.bg_fill);
    f32_row(ui, "Height", &mut s.height, 8.0..=80.0, 0.5);
    margin_ui(ui, &mut s.inner_margin, "Inner Margin");
    corner_radius_ui(ui, &mut s.corner_radius, "Radius");
    color_row(ui, "Hline Color", &mut s.hline_color);
    ui.checkbox(&mut s.fill_tab_bar, "Fill Tab Bar");
    ui.checkbox(&mut s.show_scroll_bar_on_overflow, "Show Scroll on Overflow");
}

fn tab_style_ui(ui: &mut Ui, s: &mut TabStyle) {
    tab_interaction_style_ui(ui, "Active", &mut s.active);
    tab_interaction_style_ui(ui, "Inactive", &mut s.inactive);
    tab_interaction_style_ui(ui, "Focused", &mut s.focused);
    tab_interaction_style_ui(ui, "Hovered", &mut s.hovered);
    tab_interaction_style_ui(ui, "Inactive + KB Focus", &mut s.inactive_with_kb_focus);
    tab_interaction_style_ui(ui, "Active + KB Focus", &mut s.active_with_kb_focus);
    tab_interaction_style_ui(ui, "Focused + KB Focus", &mut s.focused_with_kb_focus);
    CollapsingHeader::new("Tab Body").default_open(false).show(ui, |ui| {
        tab_body_style_ui(ui, &mut s.tab_body);
    });
    f32_row(ui, "Spacing", &mut s.spacing, -10.0..=20.0, 0.1);
    ui.checkbox(&mut s.hline_below_active_tab_name, "Hline Below Active Tab");
    option_f32_row(ui, "Min Width", &mut s.minimum_width, 60.0);
}

fn overlay_feel_ui(ui: &mut Ui, f: &mut OverlayFeel) {
    f32_row(ui, "Window Drop Coverage", &mut f.window_drop_coverage, 0.0..=1.0, 0.01);
    f32_row(ui, "Center Drop Coverage", &mut f.center_drop_coverage, 0.0..=1.0, 0.01);
    f32_row(ui, "Fade Hold Time", &mut f.fade_hold_time, 0.0..=2.0, 0.01);
    f32_row(ui, "Max Preference Time", &mut f.max_preference_time, 0.0..=2.0, 0.01);
    f32_row(ui, "Interact Expansion", &mut f.interact_expansion, 0.0..=100.0, 1.0);
}

fn leaf_highlighting_ui(ui: &mut Ui, h: &mut LeafHighlighting) {
    color_row(ui, "Color", &mut h.color);
    corner_radius_ui(ui, &mut h.corner_radius, "Radius");
    stroke_ui(ui, &mut h.stroke, "Stroke");
    f32_row(ui, "Expansion", &mut h.expansion, 0.0..=50.0, 0.5);
}

fn overlay_style_ui(ui: &mut Ui, s: &mut OverlayStyle) {
    color_row(ui, "Selection Color", &mut s.selection_color);
    f32_row(ui, "Selection Stroke W", &mut s.selection_stroke_width, 0.0..=10.0, 0.1);
    f32_row(ui, "Button Spacing", &mut s.button_spacing, 0.0..=50.0, 0.5);
    f32_row(ui, "Max Button Size", &mut s.max_button_size, 20.0..=300.0, 1.0);
    f32_row(ui, "Surface Fade Opacity", &mut s.surface_fade_opacity, 0.0..=1.0, 0.01);
    color_row(ui, "Button Color", &mut s.button_color);
    stroke_ui(ui, &mut s.button_border_stroke, "Button Border");
    ui.horizontal(|ui| {
        ui.label("Type:");
        ui.selectable_value(&mut s.overlay_type, OverlayType::HighlightedAreas, "Highlighted");
        ui.selectable_value(&mut s.overlay_type, OverlayType::Widgets, "Widgets");
    });
    CollapsingHeader::new("Overlay Feel").default_open(false).show(ui, |ui| {
        overlay_feel_ui(ui, &mut s.feel);
    });
    CollapsingHeader::new("Hovered Leaf Highlight").default_open(false).show(ui, |ui| {
        leaf_highlighting_ui(ui, &mut s.hovered_leaf_highlight);
    });
}

fn dock_style_ui(ui: &mut Ui, style: &mut DockStyle) {
    CollapsingHeader::new("📊 Tab Bar").default_open(true).show(ui, |ui| {
        tab_bar_style_ui(ui, &mut style.tab_bar);
    });
    CollapsingHeader::new("📑 Tabs").default_open(false).show(ui, |ui| {
        tab_style_ui(ui, &mut style.tab);
    });
    CollapsingHeader::new("↔️ Separator").default_open(false).show(ui, |ui| {
        separator_style_ui(ui, &mut style.separator);
    });
    CollapsingHeader::new("🔘 Buttons").default_open(false).show(ui, |ui| {
        buttons_style_ui(ui, &mut style.buttons);
    });
    CollapsingHeader::new("📐 Dock Area").default_open(false).show(ui, |ui| {
        option_margin_ui(ui, "Padding", &mut style.dock_area_padding, Margin::same(4));
        stroke_ui(ui, &mut style.main_surface_border_stroke, "Border Stroke");
        corner_radius_ui(ui, &mut style.main_surface_border_rounding, "Border Radius");
    });
    CollapsingHeader::new("🖱️ Overlay").default_open(false).show(ui, |ui| {
        overlay_style_ui(ui, &mut style.overlay);
    });
}

// ===== Public entry point =====

/// Render the full editor theme UI with two top-level sections:
/// 1. 🖌️ egui Settings — native egui visuals/spacing/widgets
/// 2. 📋 Dock Style — egui_dock appearance
///
/// Returns `true` when the user clicks "Save Theme".
pub fn show(
    ui: &mut egui::Ui,
    egui_style: &mut egui::Style,
    egui_baseline: &egui::Style,
    dock_style: &mut DockStyle,
    dock_baseline: &DockStyle,
    dock_default: &DockStyle,
) -> bool {
    let mut save_requested = false;
    ScrollArea::vertical().show(ui, |ui| {
        // --- Toolbar ---
        ui.horizontal(|ui| {
            if ui.button("🔄 Reset egui").clicked() {
                *egui_style = egui_baseline.clone();
            }
            if ui.button("🔄 Reset Dock").clicked() {
                *dock_style = dock_default.clone();
            }
            if ui.button("🎨 Reset All from Theme").clicked() {
                *egui_style = egui_baseline.clone();
                *dock_style = dock_baseline.clone();
            }
            if ui.button("💾 Save Theme").clicked() {
                save_requested = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label("Changes apply live");
            });
        });
        ui.separator();

        // ---- Section 1: egui native settings ----
        CollapsingHeader::new("🖌️ egui Settings").default_open(true).show(ui, |ui| {
            egui_style_ui(ui, egui_style);
        });

        // ---- Section 2: egui_dock style ----
        CollapsingHeader::new("📋 Dock Style").default_open(true).show(ui, |ui| {
            dock_style_ui(ui, dock_style);
        });
    });
    save_requested
}
