use std::cell::UnsafeCell;
use std::collections::HashSet;

use bevy_ecs::prelude::*;

/// Wrapper to make a `!Send + !Sync` type compatible with ECS resources.
/// The iced `Cache` is only accessed from the main thread.
pub(crate) struct ThreadLocal<T>(UnsafeCell<T>);
unsafe impl<T> Send for ThreadLocal<T> {}
unsafe impl<T> Sync for ThreadLocal<T> {}

impl<T> ThreadLocal<T> {
    pub(crate) fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }
    pub(crate) fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitterSide {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub(crate) struct EntityTreeRow {
    pub entity: Entity,
    pub name: String,
    pub depth: usize,
    pub has_children: bool,
}

/// Identifies which component field a property line corresponds to.
/// Used to route edits back to the correct ECS component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyFieldId {
    // Transform
    PositionX,
    PositionY,
    PositionZ,
    RotationX,
    RotationY,
    RotationZ,
    ScaleX,
    ScaleY,
    ScaleZ,
    // Camera
    CameraFov,
    CameraNear,
    CameraFar,
    // PointLight
    PointLightIntensity,
    PointLightColor,
    // ParallelLight
    DirLightIntensity,
    DirLightColor,
}

/// A single row in the property inspector. Section headers use `is_header: true`.
#[derive(Debug, Clone)]
pub(crate) struct PropertyField {
    pub label: String,
    /// Formatted current value (read-only display when not editing).
    pub value: String,
    /// Live draft while the field is being edited.
    pub draft: String,
    pub field_id: PropertyFieldId,
    /// When true this row is a section heading (e.g. "── Camera ──"), not editable.
    pub is_header: bool,
    /// Non-empty when the last commit for this field failed to parse.
    pub error: Option<String>,
}

/// Mutable state used to drive the iced UI each frame.
#[derive(Clone)]
pub(crate) struct EditorUiState {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub left_panel_width: f32,
    pub right_panel_width: f32,
    pub scene_bounds: Option<(f32, f32, f32, f32)>,
    pub splitter_drag_side: Option<SplitterSide>,
    pub splitter_drag_anchor_x: f32,
    pub splitter_drag_start_width: f32,
    pub entity_tree: Vec<EntityTreeRow>,
    pub expanded_entities: HashSet<Entity>,
    pub selected_entity: Option<Entity>,
    pub property_lines: Vec<PropertyField>,
    /// Which property field (index into `property_lines`) is currently being edited.
    pub editing_property_index: Option<usize>,
    /// Estimated logical height of the inspector section, used to offset
    /// the CSM preview overlay quads.
    pub inspector_height: f32,
    /// Entity whose name is currently being edited in-place in the world tree.
    pub renaming_entity: Option<Entity>,
    /// Live draft text for the active rename. Committed to the `Name`
    /// component on submit (Enter), discarded on cancel.
    pub name_draft: String,
    /// Search/filter string for the entity tree. When non-empty, only rows
    /// whose name contains this string (case-insensitive) are shown.
    pub tree_filter: String,
}

impl Default for EditorUiState {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            left_panel_width: 260.0,
            right_panel_width: 260.0,
            scene_bounds: None,
            splitter_drag_side: None,
            splitter_drag_anchor_x: 0.0,
            splitter_drag_start_width: 0.0,
            entity_tree: Vec::new(),
            expanded_entities: HashSet::new(),
            selected_entity: None,
            property_lines: Vec::new(),
            editing_property_index: None,
            inspector_height: 0.0,
            renaming_entity: None,
            name_draft: String::new(),
            tree_filter: String::new(),
        }
    }
}

/// Messages produced by iced widgets. They are drained and applied to the
/// ECS world after each UI frame.
#[derive(Debug, Clone)]
pub(crate) enum IcedMessage {
    SplitterDragStarted(SplitterSide),
    SplitterDragEnded,
    ToggleEntityExpanded(Entity),
    SelectEntity(Entity),
    /// Begin in-place rename of the given entity (double-click on its row).
    StartRenaming(Entity),
    /// Exit rename mode without committing.
    StopRenaming,
    /// Live text while the rename input is being edited.
    EntityNameInput(Entity, String),
    /// Commit the rename (Enter) for the given entity. The new name is read
    /// from `EditorUiState::name_draft` when this is processed.
    EntityNameSubmitted(Entity),
    /// Begin editing a property field (click on its value).
    StartEditingProperty(usize),
    /// Cancel property editing without committing.
    StopEditingProperty,
    /// Live text while a property value is being edited.
    PropertyValueInput(usize, String),
    /// Commit the property edit (Enter). The new value is read from
    /// `PropertyField::draft`.
    PropertyValueSubmitted(usize),
    /// Delete the currently selected entity.
    DeleteSelectedEntity,
    /// Filter the entity tree by the given search string.
    TreeFilterChanged(String),
    /// Undo the last property edit for the selected entity.
    Undo,
    /// Redo the last undone property edit.
    Redo,
}

/// Holds editor UI state and pending commands, decoupled from the raw
/// iced_wgpu renderer.
#[derive(Resource)]
pub(crate) struct EditorUiResource {
    pub ui_state: EditorUiState,
    pub pending_messages: Vec<IcedMessage>,
    /// Renames staged by the UI (`EntityNameSubmitted`) and pending write to
    /// the ECS `Name` component. Drained by `sys_apply_iced_messages`.
    pub pending_name_commits: Vec<(Entity, String)>,
    /// Property edits staged by the UI (`PropertyValueSubmitted`) and pending
    /// write to ECS components. Drained by `sys_apply_iced_messages`.
    pub pending_property_commits: Vec<(Entity, PropertyFieldId, String)>,
    /// One-shot flag set when rename mode is entered so `update_and_draw`
    /// focuses the rename input on the next UI build.
    pub focus_rename_input: bool,
    /// One-shot flag set when property editing starts so `update_and_draw`
    /// focuses the property input on the next UI build.
    pub focus_property_input: bool,
    /// Cached entity count from the last tree rebuild; the rebuild is skipped
    /// when this matches the current frame and no force-rebuild is requested.
    pub(crate) last_entity_count: usize,
    /// Set to true to force a full entity tree rebuild next frame (e.g. after
    /// expand/collapse or hierarchy change).
    pub(crate) tree_force_rebuild: bool,
    /// Errors from the last property commit, keyed by field index. Cleared
    /// each frame by `build_property_lines`.
    pub(crate) property_errors: Vec<(usize, String)>,
    /// Entities to despawn next frame (requested by Delete key).
    pub(crate) delete_entity_queue: Vec<Entity>,
    /// Undo stack: (entity, field_id, old_value, new_value).
    pub(crate) undo_stack: Vec<(Entity, PropertyFieldId, String, String)>,
    /// Redo stack, populated when undo is performed.
    pub(crate) redo_stack: Vec<(Entity, PropertyFieldId, String, String)>,
}

impl Default for EditorUiResource {
    fn default() -> Self {
        Self {
            ui_state: EditorUiState::default(),
            pending_messages: Vec::new(),
            pending_name_commits: Vec::new(),
            pending_property_commits: Vec::new(),
            focus_rename_input: false,
            focus_property_input: false,
            last_entity_count: 0,
            tree_force_rebuild: true, // build on first frame
            property_errors: Vec::new(),
            delete_entity_queue: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}
