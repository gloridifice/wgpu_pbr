use egui::{Color32, Margin, Stroke};

pub struct Frame;
pub struct ThemeColor;

impl ThemeColor {
    pub const BG2: Color32 = Color32::from_rgb(60, 60, 60);
    pub const BG1: Color32 = Color32::from_rgb(45, 45, 45);
    pub const BG0: Color32 = Color32::from_rgb(27, 27, 27);
    pub const TEXT0: Color32 = Color32::from_rgb(60, 60, 60);
    pub const TEXT1: Color32 = Color32::from_rgb(60, 60, 60);
}

impl Frame {
    pub fn new() -> egui::Frame {
        egui::Frame::new()
            .corner_radius(6.0)
            .outer_margin(Margin::same(4))
            .stroke(Stroke {
                width: 1.0,
                color: ThemeColor::BG2,
            })
            .fill(ThemeColor::BG1)
    }
}
