use iced::Color;
use iced::widget::Id;

/// Stable widget id for the in-place entity rename text field. Required so the
/// focus operation can target it the frame rename mode is entered.
pub(crate) const RENAME_INPUT_ID: Id = Id::new("entity_rename");
/// Stable widget id for the property value edit text field.
pub(crate) const PROPERTY_INPUT_ID: Id = Id::new("property_value");

// Editor UI palette — cohesive dark theme tuned to match Theme::TokyoNight.
pub(crate) const PANEL_BG: Color = Color::from_rgba(0.08, 0.09, 0.12, 0.92);
pub(crate) const PANEL_BORDER: Color = Color::from_rgb(0.20, 0.22, 0.28);
pub(crate) const HEADER_TEXT: Color = Color::from_rgb(0.82, 0.86, 0.95);
pub(crate) const MUTED_TEXT: Color = Color::from_rgb(0.52, 0.56, 0.68);
pub(crate) const BODY_TEXT: Color = Color::from_rgb(0.86, 0.88, 0.92);
pub(crate) const ACCENT: Color = Color::from_rgb(1.0, 0.84, 0.0);
pub(crate) const ROW_SELECTED_BG: Color = Color::from_rgba(0.18, 0.34, 0.62, 0.55);
pub(crate) const ROW_RENAME_BG: Color = Color::from_rgba(0.16, 0.20, 0.28, 0.85);
pub(crate) const INSPECTOR_BG: Color = Color::from_rgba(0.10, 0.11, 0.15, 0.70);
pub(crate) const INSPECTOR_BORDER: Color = Color::from_rgb(0.22, 0.24, 0.30);
pub(crate) const SPLITTER: Color = Color::from_rgb(0.24, 0.26, 0.32);
