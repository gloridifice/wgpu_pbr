use egui::*;
use egui_dock::{
    ButtonsStyle, LeafHighlighting, OverlayFeel, OverlayStyle, OverlayType, SeparatorStyle,
    Style as DockStyle, TabAddAlign, TabBarStyle, TabBodyStyle, TabInteractionStyle, TabStyle,
};

// ===== Helper widgets (control-only; label is emitted by the caller as a Grid cell) =====

fn corner_radius_controls(ui: &mut Ui, cr: &mut CornerRadius) {
    ui.horizontal(|ui| {
        if cr.is_same() {
            let mut v = cr.nw;
            if ui
                .add(
                    DragValue::new(&mut v)
                        .range(0..=255)
                        .speed(0.5)
                        .prefix("all: "),
                )
                .changed()
            {
                *cr = CornerRadius::same(v);
            }
        } else {
            for (name, field) in [
                ("NW", &mut cr.nw),
                ("NE", &mut cr.ne),
                ("SW", &mut cr.sw),
                ("SE", &mut cr.se),
            ] {
                ui.label(name);
                let mut v = *field;
                if ui
                    .add(DragValue::new(&mut v).range(0..=255).speed(0.5))
                    .changed()
                {
                    *field = v;
                }
            }
        }
    });
}

fn margin_controls(ui: &mut Ui, m: &mut Margin) {
    ui.horizontal(|ui| {
        for (name, field) in [
            ("L", &mut m.left),
            ("R", &mut m.right),
            ("T", &mut m.top),
            ("B", &mut m.bottom),
        ] {
            ui.label(name);
            let mut v = *field as f32;
            if ui
                .add(DragValue::new(&mut v).range(-128.0..=127.0).speed(0.5))
                .changed()
            {
                *field = v.round() as i8;
            }
        }
    });
}

fn option_margin_controls(ui: &mut Ui, opt: &mut Option<Margin>, default_val: Margin) {
    ui.horizontal(|ui| {
        let mut enabled = opt.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            if enabled {
                *opt = Some(default_val);
            } else {
                *opt = None;
            }
        }
        if let Some(m) = opt.as_mut() {
            margin_controls(ui, m);
        }
    });
}

fn stroke_controls(ui: &mut Ui, s: &mut Stroke) {
    ui.horizontal(|ui| {
        ui.label("W:");
        ui.add(DragValue::new(&mut s.width).range(0.0..=20.0).speed(0.1));
        ui.label("C:");
        ui.color_edit_button_srgba(&mut s.color);
    });
}

fn color_controls(ui: &mut Ui, c: &mut Color32) {
    ui.color_edit_button_srgba(c);
}

fn f32_controls(ui: &mut Ui, v: &mut f32, range: std::ops::RangeInclusive<f32>, speed: f32) {
    ui.add(DragValue::new(v).range(range).speed(speed));
}

fn option_f32_controls(ui: &mut Ui, opt: &mut Option<f32>, default_val: f32) {
    ui.horizontal(|ui| {
        let mut enabled = opt.is_some();
        if ui.checkbox(&mut enabled, "").changed() {
            if enabled {
                *opt = Some(default_val);
            } else {
                *opt = None;
            }
        }
        if let Some(v) = opt.as_mut() {
            ui.add(DragValue::new(v).speed(0.5));
        }
    });
}

// ===== Dock style editors =====

fn tab_interaction_style_ui(ui: &mut Ui, name: &str, s: &mut TabInteractionStyle) {
    CollapsingHeader::new(name)
        .default_open(false)
        .show(ui, |ui| {
            Grid::new(format!("{name}_grid")).show(ui, |ui| {
                ui.label("Outline");
                color_controls(ui, &mut s.outline_color);
                ui.end_row();

                ui.label("Radius");
                corner_radius_controls(ui, &mut s.corner_radius);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.bg_fill);
                ui.end_row();

                ui.label("Text");
                color_controls(ui, &mut s.text_color);
                ui.end_row();
            });
        });
}

fn tab_body_style_ui(ui: &mut Ui, s: &mut TabBodyStyle) {
    Grid::new("tab_body").show(ui, |ui| {
        ui.label("Inner Margin");
        margin_controls(ui, &mut s.inner_margin);
        ui.end_row();

        ui.label("Stroke");
        stroke_controls(ui, &mut s.stroke);
        ui.end_row();

        ui.label("Radius");
        corner_radius_controls(ui, &mut s.corner_radius);
        ui.end_row();

        ui.label("Bg Fill");
        color_controls(ui, &mut s.bg_fill);
        ui.end_row();
    });
}

fn buttons_style_ui(ui: &mut Ui, s: &mut ButtonsStyle) {
    CollapsingHeader::new("Add Tab Button")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("btn_add_tab").show(ui, |ui| {
                ui.label("Align");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut s.add_tab_align, TabAddAlign::Left, "Left");
                    ui.selectable_value(&mut s.add_tab_align, TabAddAlign::Right, "Right");
                });
                ui.end_row();

                ui.label("Color");
                color_controls(ui, &mut s.add_tab_color);
                ui.end_row();

                ui.label("Active Color");
                color_controls(ui, &mut s.add_tab_active_color);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.add_tab_bg_fill);
                ui.end_row();

                ui.label("Border Color");
                color_controls(ui, &mut s.add_tab_border_color);
                ui.end_row();
            });
        });
    CollapsingHeader::new("Close Tab Button")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("btn_close_tab").show(ui, |ui| {
                ui.label("Color");
                color_controls(ui, &mut s.close_tab_color);
                ui.end_row();

                ui.label("Active Color");
                color_controls(ui, &mut s.close_tab_active_color);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.close_tab_bg_fill);
                ui.end_row();
            });
        });
    CollapsingHeader::new("Close All Button")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("btn_close_all").show(ui, |ui| {
                ui.label("Color");
                color_controls(ui, &mut s.close_all_tabs_color);
                ui.end_row();

                ui.label("Active Color");
                color_controls(ui, &mut s.close_all_tabs_active_color);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.close_all_tabs_bg_fill);
                ui.end_row();

                ui.label("Border Color");
                color_controls(ui, &mut s.close_all_tabs_border_color);
                ui.end_row();

                ui.label("Disabled Color");
                color_controls(ui, &mut s.close_all_tabs_disabled_color);
                ui.end_row();
            });
        });
    CollapsingHeader::new("Collapse Button")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("btn_collapse").show(ui, |ui| {
                ui.label("Color");
                color_controls(ui, &mut s.collapse_tabs_color);
                ui.end_row();

                ui.label("Active Color");
                color_controls(ui, &mut s.collapse_tabs_active_color);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.collapse_tabs_bg_fill);
                ui.end_row();

                ui.label("Border Color");
                color_controls(ui, &mut s.collapse_tabs_border_color);
                ui.end_row();
            });
        });
    CollapsingHeader::new("Minimize Button")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("btn_minimize").show(ui, |ui| {
                ui.label("Color");
                color_controls(ui, &mut s.minimize_window_color);
                ui.end_row();

                ui.label("Active Color");
                color_controls(ui, &mut s.minimize_window_active_color);
                ui.end_row();

                ui.label("Bg Fill");
                color_controls(ui, &mut s.minimize_window_bg_fill);
                ui.end_row();

                ui.label("Border Color");
                color_controls(ui, &mut s.minimize_window_border_color);
                ui.end_row();
            });
        });
}

fn separator_style_ui(ui: &mut Ui, s: &mut SeparatorStyle) {
    Grid::new("separator").show(ui, |ui| {
        ui.label("Width");
        f32_controls(ui, &mut s.width, 0.0..=20.0, 0.1);
        ui.end_row();

        ui.label("Extra Interact");
        f32_controls(ui, &mut s.extra_interact_width, 0.0..=50.0, 0.5);
        ui.end_row();

        ui.label("Extra");
        f32_controls(ui, &mut s.extra, 0.0..=500.0, 1.0);
        ui.end_row();

        ui.label("Idle");
        color_controls(ui, &mut s.color_idle);
        ui.end_row();

        ui.label("Hovered");
        color_controls(ui, &mut s.color_hovered);
        ui.end_row();

        ui.label("Dragged");
        color_controls(ui, &mut s.color_dragged);
        ui.end_row();
    });
}

fn tab_bar_style_ui(ui: &mut Ui, s: &mut TabBarStyle) {
    Grid::new("tab_bar").show(ui, |ui| {
        ui.label("Bg Fill");
        color_controls(ui, &mut s.bg_fill);
        ui.end_row();

        ui.label("Height");
        f32_controls(ui, &mut s.height, 8.0..=80.0, 0.5);
        ui.end_row();

        ui.label("Inner Margin");
        margin_controls(ui, &mut s.inner_margin);
        ui.end_row();

        ui.label("Radius");
        corner_radius_controls(ui, &mut s.corner_radius);
        ui.end_row();

        ui.label("Hline Color");
        color_controls(ui, &mut s.hline_color);
        ui.end_row();

        ui.label("Fill Tab Bar");
        ui.checkbox(&mut s.fill_tab_bar, "");
        ui.end_row();

        ui.label("Show Scroll on Overflow");
        ui.checkbox(&mut s.show_scroll_bar_on_overflow, "");
        ui.end_row();
    });
}

fn tab_style_ui(ui: &mut Ui, s: &mut TabStyle) {
    tab_interaction_style_ui(ui, "Active", &mut s.active);
    tab_interaction_style_ui(ui, "Inactive", &mut s.inactive);
    tab_interaction_style_ui(ui, "Focused", &mut s.focused);
    tab_interaction_style_ui(ui, "Hovered", &mut s.hovered);
    tab_interaction_style_ui(ui, "Inactive + KB Focus", &mut s.inactive_with_kb_focus);
    tab_interaction_style_ui(ui, "Active + KB Focus", &mut s.active_with_kb_focus);
    tab_interaction_style_ui(ui, "Focused + KB Focus", &mut s.focused_with_kb_focus);
    CollapsingHeader::new("Tab Body")
        .default_open(false)
        .show(ui, |ui| {
            tab_body_style_ui(ui, &mut s.tab_body);
        });
    Grid::new("tab_style_misc").show(ui, |ui| {
        ui.label("Spacing");
        f32_controls(ui, &mut s.spacing, -10.0..=20.0, 0.1);
        ui.end_row();

        ui.label("Hline Below Active Tab");
        ui.checkbox(&mut s.hline_below_active_tab_name, "");
        ui.end_row();

        ui.label("Min Width");
        option_f32_controls(ui, &mut s.minimum_width, 60.0);
        ui.end_row();
    });
}

fn overlay_feel_ui(ui: &mut Ui, f: &mut OverlayFeel) {
    Grid::new("overlay_feel").show(ui, |ui| {
        ui.label("Window Drop Coverage");
        f32_controls(ui, &mut f.window_drop_coverage, 0.0..=1.0, 0.01);
        ui.end_row();

        ui.label("Center Drop Coverage");
        f32_controls(ui, &mut f.center_drop_coverage, 0.0..=1.0, 0.01);
        ui.end_row();

        ui.label("Fade Hold Time");
        f32_controls(ui, &mut f.fade_hold_time, 0.0..=2.0, 0.01);
        ui.end_row();

        ui.label("Max Preference Time");
        f32_controls(ui, &mut f.max_preference_time, 0.0..=2.0, 0.01);
        ui.end_row();

        ui.label("Interact Expansion");
        f32_controls(ui, &mut f.interact_expansion, 0.0..=100.0, 1.0);
        ui.end_row();
    });
}

fn leaf_highlighting_ui(ui: &mut Ui, h: &mut LeafHighlighting) {
    Grid::new("leaf_highlight").show(ui, |ui| {
        ui.label("Color");
        color_controls(ui, &mut h.color);
        ui.end_row();

        ui.label("Radius");
        corner_radius_controls(ui, &mut h.corner_radius);
        ui.end_row();

        ui.label("Stroke");
        stroke_controls(ui, &mut h.stroke);
        ui.end_row();

        ui.label("Expansion");
        f32_controls(ui, &mut h.expansion, 0.0..=50.0, 0.5);
        ui.end_row();
    });
}

fn overlay_style_ui(ui: &mut Ui, s: &mut OverlayStyle) {
    Grid::new("overlay").show(ui, |ui| {
        ui.label("Selection Color");
        color_controls(ui, &mut s.selection_color);
        ui.end_row();

        ui.label("Selection Stroke W");
        f32_controls(ui, &mut s.selection_stroke_width, 0.0..=10.0, 0.1);
        ui.end_row();

        ui.label("Button Spacing");
        f32_controls(ui, &mut s.button_spacing, 0.0..=50.0, 0.5);
        ui.end_row();

        ui.label("Max Button Size");
        f32_controls(ui, &mut s.max_button_size, 20.0..=300.0, 1.0);
        ui.end_row();

        ui.label("Surface Fade Opacity");
        f32_controls(ui, &mut s.surface_fade_opacity, 0.0..=1.0, 0.01);
        ui.end_row();

        ui.label("Button Color");
        color_controls(ui, &mut s.button_color);
        ui.end_row();

        ui.label("Button Border");
        stroke_controls(ui, &mut s.button_border_stroke);
        ui.end_row();

        ui.label("Type");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut s.overlay_type,
                OverlayType::HighlightedAreas,
                "Highlighted",
            );
            ui.selectable_value(&mut s.overlay_type, OverlayType::Widgets, "Widgets");
        });
        ui.end_row();
    });
    CollapsingHeader::new("Overlay Feel")
        .default_open(false)
        .show(ui, |ui| {
            overlay_feel_ui(ui, &mut s.feel);
        });
    CollapsingHeader::new("Hovered Leaf Highlight")
        .default_open(false)
        .show(ui, |ui| {
            leaf_highlighting_ui(ui, &mut s.hovered_leaf_highlight);
        });
}

fn dock_style_ui(ui: &mut Ui, style: &mut DockStyle) {
    CollapsingHeader::new("📊 Tab Bar")
        .default_open(true)
        .show(ui, |ui| {
            tab_bar_style_ui(ui, &mut style.tab_bar);
        });
    CollapsingHeader::new("📑 Tabs")
        .default_open(false)
        .show(ui, |ui| {
            tab_style_ui(ui, &mut style.tab);
        });
    CollapsingHeader::new("↔️ Separator")
        .default_open(false)
        .show(ui, |ui| {
            separator_style_ui(ui, &mut style.separator);
        });
    CollapsingHeader::new("🔘 Buttons")
        .default_open(false)
        .show(ui, |ui| {
            buttons_style_ui(ui, &mut style.buttons);
        });
    CollapsingHeader::new("📐 Dock Area")
        .default_open(false)
        .show(ui, |ui| {
            Grid::new("dock_area").show(ui, |ui| {
                ui.label("Padding");
                option_margin_controls(ui, &mut style.dock_area_padding, Margin::same(4));
                ui.end_row();

                ui.label("Border Stroke");
                stroke_controls(ui, &mut style.main_surface_border_stroke);
                ui.end_row();

                ui.label("Border Radius");
                corner_radius_controls(ui, &mut style.main_surface_border_rounding);
                ui.end_row();
            });
        });
    CollapsingHeader::new("🖱️ Overlay")
        .default_open(false)
        .show(ui, |ui| {
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
        CollapsingHeader::new("egui Settings")
            .default_open(true)
            .show(ui, |ui| {
                egui_style.ui(ui);
            });

        // ---- Section 2: egui_dock style ----
        CollapsingHeader::new("Dock Style")
            .default_open(true)
            .show(ui, |ui| {
                dock_style_ui(ui, dock_style);
            });
    });
    save_requested
}
