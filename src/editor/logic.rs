//! Pure logic functions extracted from the editor systems, suitable for
//! unit testing without GPU or ECS world dependencies.
//!
//! The functions in this module mirror those in `gui/mod.rs` but operate
//! on plain data structures so they can be called from tests.
//!
//! The free functions exist for testability; the production systems in
//! `gui/mod.rs` use inlined equivalents. Suppress the dead-code warning
//! for this module so the test surface remains available.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::editor::data_types::{
    EditorUiState, EntityTreeRow, IcedMessage, PropertyFieldId, SplitterSide,
};

/// Build the entity tree from raw query data. Returns the flat list of rows
/// in depth-first expanded order.
pub fn build_entity_tree_from_data(
    named: &[(Entity, String, Option<Entity>, bool)], // (entity, name, parent, has_children)
    expanded: &HashSet<Entity>,
) -> Vec<EntityTreeRow> {
    let mut children_map: HashMap<Entity, Vec<(Entity, String)>> = HashMap::new();
    let mut roots: Vec<(Entity, String, bool)> = Vec::new();

    for &(entity, ref name, parent, has_children) in named {
        match parent {
            Some(parent_entity) => {
                children_map
                    .entry(parent_entity)
                    .or_default()
                    .push((entity, name.clone()));
                if has_children {
                    children_map.entry(entity).or_default();
                }
            }
            None => {
                roots.push((entity, name.clone(), has_children));
            }
        }
    }

    fn collect_rows(
        entity: Entity,
        name: &str,
        depth: usize,
        has_children: bool,
        expanded: &HashSet<Entity>,
        children_map: &HashMap<Entity, Vec<(Entity, String)>>,
        rows: &mut Vec<EntityTreeRow>,
    ) {
        rows.push(EntityTreeRow {
            entity,
            name: name.to_string(),
            depth,
            has_children,
        });

        if has_children && expanded.contains(&entity) {
            if let Some(children) = children_map.get(&entity) {
                for (child_entity, child_name) in children {
                    let child_has_children = children_map.contains_key(child_entity);
                    collect_rows(
                        *child_entity,
                        child_name,
                        depth + 1,
                        child_has_children,
                        expanded,
                        children_map,
                        rows,
                    );
                }
            }
        }
    }

    let mut rows = Vec::new();
    for (entity, name, has_children) in roots {
        collect_rows(entity, &name, 0, has_children, expanded, &children_map, &mut rows);
    }
    rows
}

/// Process a batch of iced messages and update the editor state accordingly.
/// `cursor` is the current iced cursor position for splitter drag messages.
pub fn process_messages(
    state: &mut EditorUiState,
    messages: &[IcedMessage],
    cursor: Option<(f32, f32)>, // (x, y) if available
) -> (Vec<(Entity, String)>, Vec<(Entity, PropertyFieldId, String, String)>) {
    // (name_commits, property_commits_with_old_values)
    let mut name_commits = Vec::new();
    let mut prop_commits = Vec::new();

    for msg in messages {
        match msg {
            IcedMessage::SplitterDragStarted(side) => {
                if let Some((x, _)) = cursor {
                    state.splitter_drag_side = Some(*side);
                    state.splitter_drag_anchor_x = x;
                    state.splitter_drag_start_width = match side {
                        SplitterSide::Left => state.left_panel_width,
                        SplitterSide::Right => state.right_panel_width,
                    };
                }
                state.renaming_entity = None;
            }
            IcedMessage::SplitterDragEnded => {
                state.splitter_drag_side = None;
            }
            IcedMessage::ToggleEntityExpanded(entity) => {
                if state.expanded_entities.contains(entity) {
                    state.expanded_entities.remove(entity);
                } else {
                    state.expanded_entities.insert(*entity);
                }
                state.renaming_entity = None;
            }
            IcedMessage::SelectEntity(entity) => {
                state.selected_entity = Some(*entity);
                state.renaming_entity = None;
                state.editing_property_index = None;
            }
            IcedMessage::StartRenaming(entity) => {
                let current = state
                    .entity_tree
                    .iter()
                    .find(|r| &r.entity == entity)
                    .map(|r| r.name.clone())
                    .unwrap_or_default();
                state.name_draft = current;
                state.renaming_entity = Some(*entity);
            }
            IcedMessage::StopRenaming => {
                state.renaming_entity = None;
            }
            IcedMessage::EntityNameInput(entity, text) => {
                if state.renaming_entity == Some(*entity) {
                    state.name_draft = text.clone();
                }
            }
            IcedMessage::EntityNameSubmitted(entity) => {
                if state.renaming_entity == Some(*entity) {
                    let new_name = state.name_draft.trim().to_string();
                    if !new_name.is_empty() {
                        name_commits.push((*entity, new_name));
                    }
                    state.renaming_entity = None;
                }
            }
            IcedMessage::StartEditingProperty(index) => {
                let current_value = state
                    .property_lines
                    .get(*index)
                    .filter(|f| !f.is_header)
                    .map(|f| f.value.clone());
                if let Some(value) = current_value {
                    if let Some(f) = state.property_lines.get_mut(*index) {
                        f.draft = value;
                    }
                    state.editing_property_index = Some(*index);
                }
            }
            IcedMessage::StopEditingProperty => {
                state.editing_property_index = None;
            }
            IcedMessage::PropertyValueInput(index, text) => {
                if state.editing_property_index == Some(*index) {
                    if let Some(field) = state.property_lines.get_mut(*index) {
                        field.draft = text.clone();
                    }
                }
            }
            IcedMessage::PropertyValueSubmitted(index) => {
                if state.editing_property_index == Some(*index) {
                    if let Some(field) = state.property_lines.get(*index) {
                        let new_value = field.draft.trim().to_string();
                        if !new_value.is_empty() && state.selected_entity.is_some() {
                            let old = field.value.clone();
                            prop_commits.push((
                                state.selected_entity.unwrap(),
                                field.field_id,
                                old,
                                new_value,
                            ));
                        }
                    }
                    state.editing_property_index = None;
                }
            }
            // The following messages are handled by the full system; tests
            // can just verify they don't panic.
            IcedMessage::DeleteSelectedEntity
            | IcedMessage::TreeFilterChanged(_)
            | IcedMessage::Undo
            | IcedMessage::Redo => {}
        }
    }
    (name_commits, prop_commits)
}

/// Parse a color string in `"r,g,b"` or `"#RRGGBB"` hex format.
pub fn parse_color(s: &str) -> Result<(f32, f32, f32), String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return Err("Hex color must be #RRGGBB (6 hex digits)".into());
        }
        let r =
            u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex".to_string())? as f32
                / 255.0;
        let g =
            u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex".to_string())? as f32
                / 255.0;
        let b =
            u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex".to_string())? as f32
                / 255.0;
        return Ok((r, g, b));
    }
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("Use 'r,g,b' or '#RRGGBB' format".into());
    }
    let r = parts[0]
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("Invalid R: {e}"))?;
    let g = parts[1]
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("Invalid G: {e}"))?;
    let b = parts[2]
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("Invalid B: {e}"))?;
    Ok((r, g, b))
}

/// Compute new splitter widths from a drag delta.
pub fn update_splitter_drag_widths(
    current_side: Option<SplitterSide>,
    anchor_x: f32,
    start_width: f32,
    current_left: f32,
    current_right: f32,
    cursor_x: f32,
    viewport_w: f32,
) -> (f32, f32) {
    let Some(side) = current_side else {
        return (current_left, current_right);
    };
    let delta = cursor_x - anchor_x;
    let new_width = (start_width + delta).max(60.0);
    match side {
        SplitterSide::Left => {
            let max_w = viewport_w - current_right - 8.0;
            (new_width.min(max_w), current_right)
        }
        SplitterSide::Right => {
            let max_w = viewport_w - current_left - 8.0;
            (current_left, new_width.min(max_w))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::entity::Entity;

    fn e(id: u32) -> Entity {
        Entity::from_bits(id as u64)
    }

    // ------------------------------------------------------------------
    // Entity tree tests
    // ------------------------------------------------------------------

    #[test]
    fn test_empty_tree() {
        let rows = build_entity_tree_from_data(&[], &HashSet::new());
        assert!(rows.is_empty());
    }

    #[test]
    fn test_single_root() {
        let data = vec![(e(1), "Root".into(), None, false)];
        let rows = build_entity_tree_from_data(&data, &HashSet::new());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Root");
        assert_eq!(rows[0].depth, 0);
    }

    #[test]
    fn test_parent_child_collapsed() {
        // Parent (1) has child (2), but parent is NOT expanded.
        let data = vec![
            (e(1), "Parent".into(), None, true),
            (e(2), "Child".into(), Some(e(1)), false),
        ];
        let expanded: HashSet<Entity> = HashSet::new(); // nothing expanded
        let rows = build_entity_tree_from_data(&data, &expanded);
        assert_eq!(rows.len(), 1, "Child should be hidden when parent is collapsed");
        assert_eq!(rows[0].name, "Parent");
    }

    #[test]
    fn test_parent_child_expanded() {
        let data = vec![
            (e(1), "Parent".into(), None, true),
            (e(2), "Child".into(), Some(e(1)), false),
        ];
        let mut expanded = HashSet::new();
        expanded.insert(e(1));
        let rows = build_entity_tree_from_data(&data, &expanded);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Parent");
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].name, "Child");
        assert_eq!(rows[1].depth, 1);
    }

    #[test]
    fn test_nested_hierarchy() {
        // 1 -> 2 -> 3, all expanded
        let data = vec![
            (e(1), "A".into(), None, true),
            (e(2), "B".into(), Some(e(1)), true),
            (e(3), "C".into(), Some(e(2)), false),
        ];
        let mut expanded = HashSet::new();
        expanded.insert(e(1));
        expanded.insert(e(2));
        let rows = build_entity_tree_from_data(&data, &expanded);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].depth, 1);
        assert_eq!(rows[2].depth, 2);
    }

    #[test]
    fn test_multiple_roots() {
        let data = vec![
            (e(1), "R1".into(), None, false),
            (e(2), "R2".into(), None, false),
        ];
        let rows = build_entity_tree_from_data(&data, &HashSet::new());
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.depth == 0));
    }

    // ------------------------------------------------------------------
    // Message processing tests
    // ------------------------------------------------------------------

    fn default_state() -> EditorUiState {
        EditorUiState::default()
    }

    #[test]
    fn test_select_entity() {
        let mut state = default_state();
        let msgs = vec![IcedMessage::SelectEntity(e(42))];
        let (names, props) = process_messages(&mut state, &msgs, None);
        assert_eq!(state.selected_entity, Some(e(42)));
        assert!(state.renaming_entity.is_none());
        assert!(state.editing_property_index.is_none());
        assert!(names.is_empty());
        assert!(props.is_empty());
    }

    #[test]
    fn test_start_stop_rename() {
        let mut state = default_state();
        // Setup: entity tree has entity 42 named "Foo"
        state.entity_tree.push(EntityTreeRow {
            entity: e(42),
            name: "Foo".into(),
            depth: 0,
            has_children: false,
        });

        // Start rename
        let msgs = vec![IcedMessage::StartRenaming(e(42))];
        process_messages(&mut state, &msgs, None);
        assert_eq!(state.renaming_entity, Some(e(42)));
        assert_eq!(state.name_draft, "Foo");

        // Type something
        let msgs = vec![IcedMessage::EntityNameInput(e(42), "Bar".into())];
        process_messages(&mut state, &msgs, None);
        assert_eq!(state.name_draft, "Bar");

        // Submit
        let msgs = vec![IcedMessage::EntityNameSubmitted(e(42))];
        let (names, _) = process_messages(&mut state, &msgs, None);
        assert!(state.renaming_entity.is_none());
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], (e(42), "Bar".into()));
    }

    #[test]
    fn test_rename_empty_name_not_committed() {
        let mut state = default_state();
        state.renaming_entity = Some(e(1));
        state.name_draft = "   ".into(); // whitespace only

        let msgs = vec![IcedMessage::EntityNameSubmitted(e(1))];
        let (names, _) = process_messages(&mut state, &msgs, None);
        assert!(state.renaming_entity.is_none());
        assert!(names.is_empty(), "Empty name should not be committed");
    }

    #[test]
    fn test_toggle_expand() {
        let mut state = default_state();
        state.expanded_entities.insert(e(1));

        let msgs = vec![IcedMessage::ToggleEntityExpanded(e(1))];
        process_messages(&mut state, &msgs, None);
        assert!(!state.expanded_entities.contains(&e(1)), "Should collapse");

        process_messages(&mut state, &msgs, None);
        assert!(state.expanded_entities.contains(&e(1)), "Should expand again");
    }

    // ------------------------------------------------------------------
    // Splitter drag tests
    // ------------------------------------------------------------------

    #[test]
    fn test_splitter_drag_left() {
        let (l, r) = update_splitter_drag_widths(
            Some(SplitterSide::Left),
            200.0,  // anchor_x
            260.0,  // start_width
            260.0,  // current_left
            260.0,  // current_right
            300.0,  // cursor_x (dragged 100px right)
            1920.0, // viewport_w
        );
        assert!(l > 260.0, "Left panel should grow when dragged right");
        assert_eq!(r, 260.0, "Right panel unchanged");
    }

    #[test]
    fn test_splitter_drag_clamped_min() {
        let (l, _) = update_splitter_drag_widths(
            Some(SplitterSide::Left),
            200.0, // anchor_x
            260.0, // start_width
            260.0, // current_left
            260.0, // current_right
            0.0,   // cursor_x far left (delta = -200, new = 60)
            1920.0,
        );
        assert_eq!(l, 60.0, "Left panel clamped to minimum 60px");
    }

    #[test]
    fn test_no_drag_when_idle() {
        let (l, r) = update_splitter_drag_widths(
            None, // not dragging
            0.0, 0.0, 260.0, 260.0, 500.0, 1920.0,
        );
        assert_eq!(l, 260.0);
        assert_eq!(r, 260.0);
    }

    // ------------------------------------------------------------------
    // Color parsing tests
    // ------------------------------------------------------------------

    #[test]
    fn test_parse_color_rgb() {
        let (r, g, b) = parse_color("0.5, 0.25, 0.75").unwrap();
        assert!((r - 0.5).abs() < 0.001);
        assert!((g - 0.25).abs() < 0.001);
        assert!((b - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_parse_color_hex() {
        let (r, g, b) = parse_color("#FF8033").unwrap();
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 0.502).abs() < 0.01);
        assert!((b - 0.2).abs() < 0.01);
    }

    #[test]
    fn test_parse_color_invalid() {
        assert!(parse_color("not_a_color").is_err());
        assert!(parse_color("1,2").is_err());
        assert!(parse_color("#XYZ").is_err());
    }
}
