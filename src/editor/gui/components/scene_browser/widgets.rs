use crate::scene::browser::{BrowserEntry, BrowserEntryKind};
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

pub(super) const BACKGROUND: Color32 = Color32::from_rgb(24, 24, 24);
pub(super) const SURFACE: Color32 = Color32::from_rgb(34, 34, 34);
const BORDER: Color32 = Color32::from_rgb(55, 55, 55);
const TEXT: Color32 = Color32::from_rgb(220, 220, 220);
const MUTED: Color32 = Color32::from_rgb(145, 145, 145);
const BLUE: Color32 = Color32::from_rgb(53, 136, 225);
const GOLD: Color32 = Color32::from_rgb(200, 158, 71);

pub(super) fn style(ui: &mut Ui) {
    let style = ui.style_mut();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(7.0, 3.0);
    style.spacing.interact_size.y = 22.0;
    style.wrap_mode = Some(egui::TextWrapMode::Truncate);
    let visuals = &mut style.visuals;
    visuals.override_text_color = Some(TEXT);
    visuals.panel_fill = BACKGROUND;
    visuals.faint_bg_color = SURFACE;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.selection.bg_fill = Color32::from_rgb(34, 75, 119);
    visuals.selection.stroke = Stroke::new(1.0_f32, BLUE);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.inactive.weak_bg_fill = SURFACE;
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(48, 48, 48);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_gray(85));
}

// Vector icons remain legible with the editor's custom font and need no GPU assets.
fn icon(ui: &Ui, rect: Rect, kind: BrowserEntryKind) {
    let point = |x, y| rect.min + egui::vec2(rect.width() * x, rect.height() * y);
    let painter = ui.painter();
    match kind {
        BrowserEntryKind::Folder => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    point(0.05, 0.22),
                    point(0.40, 0.22),
                    point(0.50, 0.35),
                    point(0.95, 0.35),
                    point(0.95, 0.87),
                    point(0.05, 0.87),
                ],
                Color32::from_rgb(146, 109, 43),
                Stroke::NONE,
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    point(0.05, 0.43),
                    point(0.95, 0.43),
                    point(0.88, 0.91),
                    point(0.12, 0.91),
                ],
                GOLD,
                Stroke::NONE,
            ));
            painter.line_segment(
                [point(0.09, 0.45), point(0.91, 0.45)],
                Stroke::new(1.0_f32, Color32::from_rgb(227, 188, 105)),
            );
        }
        BrowserEntryKind::Scene => {
            painter.rect_filled(
                Rect::from_min_max(point(0.17, 0.08), point(0.83, 0.94)),
                3.0,
                Color32::from_rgb(44, 83, 120),
            );
            let line = Stroke::new(
                (rect.width() / 36.0).clamp(1.0, 2.0),
                Color32::from_rgb(148, 204, 248),
            );
            let top = [
                point(0.50, 0.27),
                point(0.73, 0.41),
                point(0.50, 0.55),
                point(0.27, 0.41),
            ];
            painter.add(egui::Shape::closed_line(top.to_vec(), line));
            painter.add(egui::Shape::line(
                vec![
                    top[3],
                    point(0.27, 0.68),
                    point(0.50, 0.82),
                    point(0.73, 0.68),
                    top[1],
                ],
                line,
            ));
            painter.line_segment([top[2], point(0.50, 0.82)], line);
        }
    }
}

fn text(ui: &Ui, value: &str, pos: Pos2, width: f32, size: f32, color: Color32) {
    let mut job = egui::text::LayoutJob::simple(
        value.into(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(pos, galley, color);
}

pub(super) fn folder_row(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(1.0), 22.0),
        Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            2.0,
            if selected {
                Color32::from_rgb(34, 65, 96)
            } else {
                SURFACE
            },
        );
    }
    icon(
        ui,
        Rect::from_min_size(rect.min + egui::vec2(4.0, 1.0), egui::vec2(21.0, 19.0)),
        BrowserEntryKind::Folder,
    );
    text(
        ui,
        label,
        rect.min + egui::vec2(31.0, 3.0),
        rect.width() - 34.0,
        12.0,
        TEXT,
    );
    response
}

pub(super) fn card(
    ui: &mut Ui,
    entry: &BrowserEntry,
    width: f32,
    selected: bool,
    current: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, width + 42.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            &entry.display_name,
        )
    });
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let fill = if selected {
            Color32::from_rgb(35, 65, 99)
        } else if response.hovered() {
            Color32::from_rgb(47, 47, 47)
        } else {
            SURFACE
        };
        painter.rect_filled(rect, 3.0, fill);
        let preview =
            Rect::from_min_size(rect.min + egui::vec2(5.0, 5.0), Vec2::splat(width - 10.0));
        painter.rect_filled(preview, 2.0, Color32::from_rgb(28, 28, 28));
        icon(
            ui,
            Rect::from_center_size(preview.center(), Vec2::splat(width * 0.72)),
            entry.kind,
        );
        let name = if entry.kind == BrowserEntryKind::Scene {
            &entry.display_name[..entry.display_name.len() - ".scn.ron".len()]
        } else {
            &entry.display_name
        };
        text(
            ui,
            name,
            rect.min + egui::vec2(7.0, width + 2.0),
            width - 14.0,
            12.0,
            TEXT,
        );
        let (kind, accent) = match entry.kind {
            BrowserEntryKind::Folder => ("Folder", GOLD),
            BrowserEntryKind::Scene => ("Scene", BLUE),
        };
        text(
            ui,
            kind,
            rect.min + egui::vec2(7.0, width + 20.0),
            width - 14.0,
            10.0,
            MUTED,
        );
        painter.rect_filled(
            Rect::from_min_max(
                rect.left_bottom() - egui::vec2(0.0, 3.0),
                rect.right_bottom(),
            ),
            0.0,
            accent,
        );
        if current {
            painter.text(
                preview.right_top() + egui::vec2(-4.0, 3.0),
                Align2::RIGHT_TOP,
                "OPEN",
                FontId::proportional(9.0),
                BLUE,
            );
        }
        painter.rect_stroke(
            rect,
            3.0,
            Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                if selected {
                    BLUE
                } else if response.hovered() {
                    Color32::from_gray(90)
                } else {
                    BORDER
                },
            ),
            StrokeKind::Inside,
        );
    }
    response.on_hover_text(format!(
        "{}{}",
        if current { "Current scene\n" } else { "" },
        entry.path.display()
    ))
}
