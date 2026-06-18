use iced::widget::{container, row, text, scrollable, text_input, Space, Column, Container, MouseArea};
use iced::mouse::Interaction;
use iced::{Background, Border, Color, Element, Length, Theme};

use crate::editor::data_types::{EditorUiState, IcedMessage, SplitterSide};
use crate::editor::palette::*;

/// Build the full editor UI element tree from the current state.
///
/// Three-pane layout with draggable splitter handles between panels.
pub(crate) fn build_editor_ui(ui_state: &EditorUiState) -> Element<'_, IcedMessage> {
    let left_w = ui_state.left_panel_width.max(60.0);
    let right_w = ui_state.right_panel_width.max(60.0);

    let left_panel = {
        let header = row![
            text("World Tree").size(15).color(HEADER_TEXT),
            Space::new().width(Length::Fill),
            text(format!(
                "{:.0} FPS · {:.2} ms",
                ui_state.fps, ui_state.frame_time_ms
            ))
            .size(11)
            .color(MUTED_TEXT),
        ]
        .align_y(iced::Alignment::Center);

        // Search/filter text input.
        let filter_input = text_input("Filter…", &ui_state.tree_filter)
            .size(11)
            .padding([2.0, 4.0])
            .on_input(IcedMessage::TreeFilterChanged);

        // Entity tree rows. Each row: indent | expand arrow | name (or
        // text_input while renaming). Double-click a name to rename it.
        let mut entity_col = Column::new().spacing(2);
        let filter_lower = ui_state.tree_filter.to_lowercase();
        for row in &ui_state.entity_tree {
            // Apply tree filter (case-insensitive substring match).
            if !filter_lower.is_empty()
                && !row.name.to_lowercase().contains(&filter_lower)
            {
                continue;
            }
            let indent = row.depth as f32 * 14.0;
            let is_selected = ui_state.selected_entity == Some(row.entity);
            let is_renaming = ui_state.renaming_entity == Some(row.entity);

            let arrow_text = if row.has_children {
                if ui_state.expanded_entities.contains(&row.entity) {
                    "▾"
                } else {
                    "▸"
                }
            } else {
                "•"
            };

            let arrow = MouseArea::new(text(arrow_text).size(12).color(MUTED_TEXT))
                .on_press(IcedMessage::ToggleEntityExpanded(row.entity));

            let name_elem: Element<'_, IcedMessage> = if is_renaming {
                text_input("rename…", &ui_state.name_draft)
                    .id(RENAME_INPUT_ID)
                    .size(12)
                    .padding([2.0, 4.0])
                    .on_input(move |s| IcedMessage::EntityNameInput(row.entity, s))
                    .on_submit(IcedMessage::EntityNameSubmitted(row.entity))
                    .into()
            } else {
                MouseArea::new(
                    text(row.name.clone())
                        .size(13)
                        .color(if is_selected { ACCENT } else { BODY_TEXT }),
                )
                .on_press(IcedMessage::SelectEntity(row.entity))
                .on_double_click(IcedMessage::StartRenaming(row.entity))
                .into()
            };

            let row_content = row![
                Space::new().width(Length::Fixed(indent)),
                arrow,
                name_elem,
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4);

            let row_bg = if is_renaming {
                ROW_RENAME_BG
            } else if is_selected {
                ROW_SELECTED_BG
            } else {
                Color::TRANSPARENT
            };

            entity_col = entity_col.push(
                container(row_content)
                    .width(Length::Fill)
                    .padding([2.0, 4.0])
                    .style(move |_theme: &Theme| container::Style {
                        background: Some(Background::Color(row_bg)),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    }),
            );
        }

        let tree_scroll = scrollable(entity_col)
            .width(Length::Fill)
            .height(Length::Fill);

        let tree_col = Column::new()
            .push(header)
            .push(filter_input)
            .push(iced::widget::rule::horizontal(1.0))
            .push(tree_scroll)
            .spacing(8);

        container(tree_col)
            .width(Length::Fixed(left_w))
            .height(Length::Fill)
            .padding(10)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(PANEL_BG)),
                border: Border::default()
                    .rounded(6.0)
                    .color(PANEL_BORDER)
                    .width(1.0),
                ..Default::default()
            })
    };

    let scene_region = Container::new(text(""))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme| container::Style::default());

    // ---- Inspector section (editable properties) ----
    let inspector_section: Element<'_, IcedMessage> =
        if ui_state.selected_entity.is_some() && !ui_state.property_lines.is_empty() {
            let mut prop_col = Column::new().spacing(3);
            for (i, field) in ui_state.property_lines.iter().enumerate() {
                let is_editing =
                    ui_state.editing_property_index == Some(i);

                if field.is_header {
                    // Section header row (e.g. "── Camera ──")
                    prop_col = prop_col.push(
                        text(field.label.clone())
                            .size(11)
                            .color(MUTED_TEXT),
                    );
                } else {
                    let label = text(format!("{}:", field.label))
                        .size(11)
                        .color(MUTED_TEXT)
                        .width(Length::Fixed(85.0));

                    let value_elem: Element<'_, IcedMessage> = if is_editing {
                        text_input("", &field.draft)
                            .id(PROPERTY_INPUT_ID)
                            .size(11)
                            .padding([1.0, 3.0])
                            .on_input(move |s| IcedMessage::PropertyValueInput(i, s))
                            .on_submit(IcedMessage::PropertyValueSubmitted(i))
                            .into()
                    } else {
                        MouseArea::new(
                            text(field.value.clone())
                                .size(11)
                                .color(BODY_TEXT),
                        )
                        .on_press(IcedMessage::StartEditingProperty(i))
                        .into()
                    };

                    let mut row_content: Vec<Element<'_, IcedMessage>> = vec![
                        label.into(),
                        value_elem.into(),
                    ];

                    // Show parse error in red below the field.
                    if let Some(ref err) = field.error {
                        row_content.push(
                            text(err.clone())
                                .size(10)
                                .color(Color::from_rgb(1.0, 0.3, 0.3))
                                .into(),
                        );
                    }

                    prop_col = prop_col.push(
                        row(row_content)
                            .align_y(iced::Alignment::Center)
                            .spacing(4),
                    );
                }
            }
            container(
                scrollable(prop_col)
                    .width(Length::Fill)
                    .height(Length::Fixed(ui_state.inspector_height.max(80.0).min(350.0))),
            )
            .width(Length::Fill)
            .padding(6)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(INSPECTOR_BG)),
                border: Border::default()
                    .rounded(4.0)
                    .color(INSPECTOR_BORDER)
                    .width(1.0),
                ..Default::default()
            })
            .into()
        } else {
            container(
                text("Select an entity to inspect its properties")
                    .size(11)
                    .color(MUTED_TEXT),
            )
            .width(Length::Fill)
            .padding(8)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(INSPECTOR_BG)),
                border: Border::default()
                    .rounded(4.0)
                    .color(INSPECTOR_BORDER)
                    .width(1.0),
                ..Default::default()
            })
            .into()
        };

    let right_panel = container(
        Column::new()
            .push(text("Inspector").size(15).color(HEADER_TEXT))
            .push(iced::widget::rule::horizontal(1.0))
            .push(inspector_section)
            .push(Space::new().height(8.0))
            .push(text("CSM Preview").size(15).color(HEADER_TEXT))
            .push(iced::widget::rule::horizontal(1.0))
            .push(Space::new().height(Length::Fill))
            .spacing(8),
    )
    .width(Length::Fixed(right_w))
    .height(Length::Fill)
    .padding(10)
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(PANEL_BG)),
        border: Border::default()
            .rounded(6.0)
            .color(PANEL_BORDER)
            .width(1.0),
        ..Default::default()
    });

    let splitter_handle = |side: SplitterSide| {
        MouseArea::new(
            container(Space::new().height(Length::Fill))
                .width(6.0)
                .height(Length::Fill)
                .style(|_theme: &Theme| container::Style {
                    background: Some(Background::Color(SPLITTER)),
                    border: Border::default().rounded(3.0),
                    ..Default::default()
                }),
        )
        .on_press(IcedMessage::SplitterDragStarted(side))
        .on_release(IcedMessage::SplitterDragEnded)
        .interaction(Interaction::ResizingHorizontally)
    };

    row![
        left_panel,
        splitter_handle(SplitterSide::Left),
        scene_region,
        splitter_handle(SplitterSide::Right),
        right_panel,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
