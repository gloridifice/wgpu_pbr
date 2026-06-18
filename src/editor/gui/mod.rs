use std::collections::HashSet;

use bevy_app::{Plugin, Update};
use bevy_ecs::prelude::*;
use lentille_core::{
    input::Input,
    time::Time,
    window::{PrimaryWindow, WinitWindow, WinitWindowEvent},
};
use lentille_render::{
    FrameSets, RenderState, SurfaceState,
    app_ext::AppExt,
    camera::{Camera, RenderTarget, TargetType},
    light::{parallel_light::ParallelLight, point_light::PointLight},
    prelude::*,
    shadow_mapping::csm::CascadeShadowMapping,
};
use wgpu::Extent3d;
use winit::keyboard::KeyCode;

use crate::control::{ControlState, EditorInputSet};
use crate::editor::data_types::{
    EditorUiResource, EntityTreeRow, IcedMessage, PropertyField, PropertyFieldId, SplitterSide,
};
use crate::editor::iced_renderer::IcedRenderer;
use crate::editor::preview_blit::{self, PreviewBlitResources};
use crate::editor::gui::components::depth_to_rgba::DepthToRgbaConverter;

pub mod components;

/// Holds GPU resources for CSM preview rendering, decoupled from the iced
/// renderer.
/// A cached CSM preview output with its pre-created bind group for the blit
/// pass, avoiding per-frame bind group creation.
type CsmOutput = (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::BindGroup,
    u32,
    u32,
);

#[derive(Resource)]
pub(crate) struct PreviewState {
    pub depth_to_rgba: DepthToRgbaConverter,
    pub csm_outputs: Vec<CsmOutput>,
    pub preview_blit: PreviewBlitResources,
}

impl FromWorld for PreviewState {
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        let rs = world.resource::<RenderState>();
        let preview_blit =
            preview_blit::create_preview_blit_resources(&rs.device, &rs.queue);
        Self {
            depth_to_rgba: DepthToRgbaConverter::new(&rs.device),
            csm_outputs: Vec::new(),
            preview_blit,
        }
    }
}

pub(crate) struct EditorGuiPlugin;

impl Plugin for EditorGuiPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(IcedRendererPlugin)
            .add_systems(
                Update,
                sys_apply_iced_messages.after(sys_sync_ui_state),
            )
            .add_render_system_in_frame_set(FrameSets::Present, sys_iced_present);
    }
}

// ===== IcedRenderer plugin =====

pub(crate) struct IcedRendererPlugin;

impl Plugin for IcedRendererPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(Update, sys_sync_ui_state.in_set(EditorInputSet))
            .init_render_resource::<IcedRenderer>()
            .init_render_resource::<PreviewState>()
            .init_resource::<EditorUiResource>()
            .add_observer(sys_handle_input);
    }
}

fn sys_handle_input(
    trigger: On<WinitWindowEvent>,
    mut iced: ResMut<IcedRenderer>,
    q_window: Query<&WinitWindow>,
) {
    let Some(window) = q_window.iter().find(|it| it.0.id() == trigger.window_id) else {
        return;
    };

    iced.handle_window_event(&window.0, &trigger.window_event);
}

fn sys_sync_ui_state(
    iced: Res<IcedRenderer>,
    mut editor: ResMut<EditorUiResource>,
    time: Option<Res<Time>>,
    mut input: ResMut<Input>,
    mut control_state: ResMut<ControlState>,
    q_named: Query<(Entity, &Name, &Transform)>,
    q_camera: Query<&Camera>,
    q_plight: Query<&PointLight>,
    q_dlight: Query<&ParallelLight>,
    q_world: Query<&WorldTransform>,
) {
    // Reset the escape-consumed flag each frame. The editor sets it below if
    // it consumes Escape; if not, the control system below may consume it.
    control_state.escape_consumed_by_editor = false;

    if let Some(time) = time {
        editor.ui_state.fps = time.fps;
        editor.ui_state.frame_time_ms = time.frame_time_ms;
    }

    // Escape cancels an in-progress rename or property edit before messages are processed.
    if editor.ui_state.renaming_entity.is_some() && input.is_key_down(KeyCode::Escape) {
        editor.pending_messages.push(IcedMessage::StopRenaming);
        control_state.escape_consumed_by_editor = true;
    }
    if editor.ui_state.editing_property_index.is_some() && input.is_key_down(KeyCode::Escape) {
        editor.pending_messages.push(IcedMessage::StopEditingProperty);
        control_state.escape_consumed_by_editor = true;
    }

    // Delete key removes the selected entity (when not renaming/editing).
    if editor.ui_state.selected_entity.is_some()
        && editor.ui_state.renaming_entity.is_none()
        && editor.ui_state.editing_property_index.is_none()
        && control_state.is_over_scene
        && input.is_key_down(KeyCode::Delete)
    {
        editor.pending_messages.push(IcedMessage::DeleteSelectedEntity);
    }

    // Ctrl+Z / Ctrl+Y for undo/redo (only when over the scene, not typing in
    // a text input).
    let ctrl = input.is_key_down(KeyCode::ControlLeft)
        || input.is_key_down(KeyCode::ControlRight);
    if ctrl && input.is_key_down(KeyCode::KeyZ) && control_state.is_over_scene {
        editor.pending_messages.push(IcedMessage::Undo);
    }
    if ctrl && input.is_key_down(KeyCode::KeyY) && control_state.is_over_scene {
        editor.pending_messages.push(IcedMessage::Redo);
    }

    process_pending_messages(&mut editor, iced.cursor);
    update_splitter_drag(&mut editor, iced.cursor, iced.viewport.logical_size().width);
    build_entity_tree(&mut editor, &q_named);
    build_property_lines(&mut editor, &q_camera, &q_plight, &q_dlight, &q_named, &q_world);

    let viewport_w = iced.viewport.logical_size().width;
    let viewport_h = iced.viewport.logical_size().height;

    let scene_x0 = editor.ui_state.left_panel_width + 4.0;
    let scene_x1 = (viewport_w - editor.ui_state.right_panel_width - 4.0).max(scene_x0);
    editor.ui_state.scene_bounds = Some((scene_x0, 0.0, scene_x1, viewport_h));

    let over_scene = match iced.cursor {
        iced::mouse::Cursor::Available(point)
        | iced::mouse::Cursor::Levitating(point) => {
            point.x >= scene_x0 && point.x <= scene_x1
                && point.y >= 0.0 && point.y <= viewport_h
        }
        iced::mouse::Cursor::Unavailable => false,
    };
    control_state.is_over_scene = over_scene;

    if over_scene && iced.events.iter().any(|e| matches!(e, iced::Event::Mouse(_))) {
        input.cursor_position = match iced.cursor {
            iced::mouse::Cursor::Available(point)
            | iced::mouse::Cursor::Levitating(point) => {
                Vec2::new(point.x, point.y)
            }
            iced::mouse::Cursor::Unavailable => input.cursor_position,
        };
    }
}

fn process_pending_messages(
    editor: &mut EditorUiResource,
    cursor: iced::mouse::Cursor,
) {
    let messages: Vec<_> = editor.pending_messages.drain(..).collect();

    for msg in messages {
        match msg {
            IcedMessage::SplitterDragStarted(side) => {
                if let iced::mouse::Cursor::Available(point) = cursor {
                    editor.ui_state.splitter_drag_side = Some(side);
                    editor.ui_state.splitter_drag_anchor_x = point.x;
                    editor.ui_state.splitter_drag_start_width = match side {
                        SplitterSide::Left => editor.ui_state.left_panel_width,
                        SplitterSide::Right => editor.ui_state.right_panel_width,
                    };
                }
                editor.ui_state.renaming_entity = None;
            }
            IcedMessage::SplitterDragEnded => {
                editor.ui_state.splitter_drag_side = None;
            }
            IcedMessage::ToggleEntityExpanded(entity) => {
                if editor.ui_state.expanded_entities.contains(&entity) {
                    editor.ui_state.expanded_entities.remove(&entity);
                } else {
                    editor.ui_state.expanded_entities.insert(entity);
                }
                editor.ui_state.renaming_entity = None;
                editor.tree_force_rebuild = true;
            }
            IcedMessage::SelectEntity(entity) => {
                editor.ui_state.selected_entity = Some(entity);
                editor.ui_state.renaming_entity = None;
                editor.ui_state.editing_property_index = None;
            }
            IcedMessage::StartRenaming(entity) => {
                let current = editor
                    .ui_state
                    .entity_tree
                    .iter()
                    .find(|r| r.entity == entity)
                    .map(|r| r.name.clone())
                    .unwrap_or_default();
                editor.ui_state.name_draft = current;
                editor.ui_state.renaming_entity = Some(entity);
                editor.focus_rename_input = true;
            }
            IcedMessage::StopRenaming => {
                editor.ui_state.renaming_entity = None;
            }
            IcedMessage::EntityNameInput(entity, text) => {
                if editor.ui_state.renaming_entity == Some(entity) {
                    editor.ui_state.name_draft = text;
                }
            }
            IcedMessage::EntityNameSubmitted(entity) => {
                if editor.ui_state.renaming_entity == Some(entity) {
                    let new_name = editor.ui_state.name_draft.trim().to_string();
                    if !new_name.is_empty() {
                        editor.pending_name_commits.push((entity, new_name));
                    }
                    editor.ui_state.renaming_entity = None;
                }
            }
            IcedMessage::StartEditingProperty(index) => {
                if let Some(field) = editor.ui_state.property_lines.get(index) {
                    if !field.is_header {
                        editor.ui_state.property_lines[index].draft = field.value.clone();
                        editor.ui_state.editing_property_index = Some(index);
                        editor.focus_property_input = true;
                    }
                }
            }
            IcedMessage::StopEditingProperty => {
                editor.ui_state.editing_property_index = None;
            }
            IcedMessage::PropertyValueInput(index, text) => {
                if editor.ui_state.editing_property_index == Some(index) {
                    if let Some(field) = editor.ui_state.property_lines.get_mut(index) {
                        field.draft = text;
                    }
                }
            }
            IcedMessage::PropertyValueSubmitted(index) => {
                if editor.ui_state.editing_property_index == Some(index) {
                    if let Some(field) = editor.ui_state.property_lines.get(index) {
                        let new_value = field.draft.trim().to_string();
                        if !new_value.is_empty() {
                            if let Some(entity) = editor.ui_state.selected_entity {
                                // Record old value for undo before staging commit.
                                let old_value = field.value.clone();
                                editor
                                    .undo_stack
                                    .push((entity, field.field_id, old_value, new_value.clone()));
                                editor.redo_stack.clear();
                                editor.pending_property_commits.push((
                                    entity,
                                    field.field_id,
                                    new_value,
                                ));
                            }
                        }
                    }
                    editor.ui_state.editing_property_index = None;
                }
            }
            IcedMessage::DeleteSelectedEntity => {
                if let Some(entity) = editor.ui_state.selected_entity.take() {
                    editor.delete_entity_queue.push(entity);
                    editor.tree_force_rebuild = true;
                }
            }
            IcedMessage::TreeFilterChanged(filter) => {
                editor.ui_state.tree_filter = filter;
            }
            IcedMessage::Undo => {
                if let Some((entity, field_id, old_value, new_value)) =
                    editor.undo_stack.pop()
                {
                    editor.redo_stack.push((
                        entity,
                        field_id,
                        old_value.clone(),
                        new_value,
                    ));
                    editor.pending_property_commits.push((
                        entity,
                        field_id,
                        old_value,
                    ));
                }
            }
            IcedMessage::Redo => {
                if let Some((entity, field_id, old_value, new_value)) =
                    editor.redo_stack.pop()
                {
                    editor.undo_stack.push((
                        entity,
                        field_id,
                        old_value,
                        new_value.clone(),
                    ));
                    editor.pending_property_commits.push((
                        entity,
                        field_id,
                        new_value,
                    ));
                }
            }
        }
    }
}

/// Drains UI-staged renames and property edits and writes them to ECS components.
/// Runs after `sys_sync_ui_state` so the staged commits are visible.
fn sys_apply_iced_messages(
    mut commands: Commands,
    mut editor: ResMut<EditorUiResource>,
    mut names: Query<&mut Name>,
    mut transforms: Query<&mut Transform>,
    mut cameras: Query<&mut Camera>,
    mut plights: Query<&mut PointLight>,
    mut dlights: Query<&mut ParallelLight>,
) {
    // Handle entity deletion.
    for entity in editor.delete_entity_queue.drain(..) {
        commands.entity(entity).despawn();
    }

    let name_commits = std::mem::take(&mut editor.pending_name_commits);
    let had_name_changes = !name_commits.is_empty();
    for (entity, new_name) in name_commits {
        if let Ok(mut name) = names.get_mut(entity) {
            name.set(new_name);
        }
    }
    if had_name_changes {
        editor.tree_force_rebuild = true;
    }

    let prop_commits = std::mem::take(&mut editor.pending_property_commits);
    editor.property_errors.clear();
    for (entity, field_id, value_str) in prop_commits {
        if let Err(err) = apply_property_commit(
            entity,
            field_id,
            &value_str,
            &mut transforms,
            &mut cameras,
            &mut plights,
            &mut dlights,
        ) {
            // Find the field index (if still present) to attach the error.
            let idx = editor
                .ui_state
                .property_lines
                .iter()
                .position(|f| f.field_id == field_id && !f.is_header);
            if let Some(idx) = idx {
                editor.property_errors.push((idx, err));
            }
        }
    }
}

/// Parse a color string in `"r,g,b"` or `"#RRGGBB"` hex format.
fn parse_color(s: &str) -> Result<(f32, f32, f32), String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return Err("Hex color must be #RRGGBB (6 hex digits)".into());
        }
        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| "Invalid hex".to_string())? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| "Invalid hex".to_string())? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| "Invalid hex".to_string())? as f32 / 255.0;
        return Ok((r, g, b));
    }
    // Try comma-separated floats.
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err("Use 'r,g,b' or '#RRGGBB' format".into());
    }
    let r = parts[0].trim().parse::<f32>().map_err(|e| format!("Invalid R: {e}"))?;
    let g = parts[1].trim().parse::<f32>().map_err(|e| format!("Invalid G: {e}"))?;
    let b = parts[2].trim().parse::<f32>().map_err(|e| format!("Invalid B: {e}"))?;
    Ok((r, g, b))
}

/// Parse a property value string and write it to the matching ECS component.
/// Returns `Err(msg)` when parsing fails.
fn apply_property_commit(
    entity: Entity,
    field_id: PropertyFieldId,
    value_str: &str,
    transforms: &mut Query<&mut Transform>,
    cameras: &mut Query<&mut Camera>,
    plights: &mut Query<&mut PointLight>,
    dlights: &mut Query<&mut ParallelLight>,
) -> Result<(), String> {
    use PropertyFieldId::*;

    match field_id {
        // --- Transform ---
        PositionX | PositionY | PositionZ => {
            let mut t = transforms.get_mut(entity).map_err(|_| "Entity not found".to_string())?;
            let v = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            match field_id {
                PositionX => t.position.x = v,
                PositionY => t.position.y = v,
                PositionZ => t.position.z = v,
                _ => {}
            }
        }
        RotationX | RotationY | RotationZ => {
            let mut t = transforms.get_mut(entity).map_err(|_| "Entity not found".to_string())?;
            let deg = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            let rad: Rad<f32> = Deg(deg).into();
            let mut euler: Euler<Rad<f32>> = Euler::from(t.rotation);
            match field_id {
                RotationX => euler.x = rad,
                RotationY => euler.y = rad,
                RotationZ => euler.z = rad,
                _ => {}
            }
            t.rotation = Quat::from(euler);
        }
        ScaleX | ScaleY | ScaleZ => {
            let mut t = transforms.get_mut(entity).map_err(|_| "Entity not found".to_string())?;
            let v = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            match field_id {
                ScaleX => t.scale.x = v,
                ScaleY => t.scale.y = v,
                ScaleZ => t.scale.z = v,
                _ => {}
            }
        }
        // --- Camera ---
        CameraFov | CameraNear | CameraFar => {
            let mut cam = cameras.get_mut(entity).map_err(|_| "Camera not found".to_string())?;
            let v = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            match field_id {
                CameraFov => cam.fovy = v,
                CameraNear => cam.znear = v,
                CameraFar => cam.zfar = v,
                _ => {}
            }
        }
        // --- PointLight ---
        PointLightIntensity => {
            let mut pl = plights.get_mut(entity).map_err(|_| "PointLight not found".to_string())?;
            let v = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            pl.intensity = v;
        }
        PointLightColor => {
            let mut pl = plights.get_mut(entity).map_err(|_| "PointLight not found".to_string())?;
            let (r, g, b) = parse_color(value_str)?;
            pl.color = Color::new(r, g, b, 1.0);
        }
        // --- ParallelLight ---
        DirLightIntensity => {
            let mut dl = dlights.get_mut(entity).map_err(|_| "DirLight not found".to_string())?;
            let v = value_str.parse::<f32>().map_err(|e| format!("Invalid float: {e}"))?;
            dl.intensity = v;
        }
        DirLightColor => {
            let mut dl = dlights.get_mut(entity).map_err(|_| "DirLight not found".to_string())?;
            let (r, g, b) = parse_color(value_str)?;
            dl.color = Color::new(r, g, b, 1.0);
        }
    }
    Ok(())
}

fn update_splitter_drag(
    editor: &mut EditorUiResource,
    cursor: iced::mouse::Cursor,
    viewport_w: f32,
) {
    if let Some(side) = editor.ui_state.splitter_drag_side {
        if let iced::mouse::Cursor::Available(point) = cursor {
            let delta = point.x - editor.ui_state.splitter_drag_anchor_x;
            let new_width = (editor.ui_state.splitter_drag_start_width + delta).max(60.0);
            match side {
                SplitterSide::Left => {
                    let max_w = viewport_w - editor.ui_state.right_panel_width - 8.0;
                    editor.ui_state.left_panel_width = new_width.min(max_w);
                }
                SplitterSide::Right => {
                    let max_w = viewport_w - editor.ui_state.left_panel_width - 8.0;
                    editor.ui_state.right_panel_width = new_width.min(max_w);
                }
            }
        }
    }
}

fn build_entity_tree(
    editor: &mut EditorUiResource,
    q_named: &Query<(Entity, &Name, &Transform)>,
) {
    // Quick change check: skip rebuild when the entity count is stable and
    // no explicit force-rebuild was requested (expand/collapse, rename, etc.).
    let entity_count = q_named.iter().count();
    if entity_count == editor.last_entity_count && !editor.tree_force_rebuild {
        return;
    }
    editor.last_entity_count = entity_count;
    editor.tree_force_rebuild = false;

    let mut children_map: std::collections::HashMap<Entity, Vec<(Entity, String)>> =
        std::collections::HashMap::new();
    let mut roots: Vec<(Entity, String, bool)> = Vec::new();

    for (entity, name, transform) in q_named.iter() {
        let name_str = name.to_string();
        let has_children = !transform.children.is_empty();

        match transform.parent {
            Some(parent) => {
                children_map
                    .entry(parent)
                    .or_default()
                    .push((entity, name_str));
                if has_children {
                    children_map.entry(entity).or_default();
                }
            }
            None => {
                roots.push((entity, name_str, has_children));
            }
        }
    }

    fn collect_rows(
        entity: Entity,
        name: &str,
        depth: usize,
        has_children: bool,
        expanded: &HashSet<Entity>,
        children_map: &std::collections::HashMap<Entity, Vec<(Entity, String)>>,
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

    let expanded = &editor.ui_state.expanded_entities;
    editor.ui_state.entity_tree.clear();
    for (entity, name, has_children) in &roots {
        collect_rows(
            *entity,
            name,
            0,
            *has_children,
            expanded,
            &children_map,
            &mut editor.ui_state.entity_tree,
        );
    }
}

fn sys_iced_present(
    mut iced: ResMut<IcedRenderer>,
    mut editor: ResMut<EditorUiResource>,
    mut preview: ResMut<PreviewState>,
    window: Single<(&WinitWindow, &SurfaceState), With<PrimaryWindow>>,
    q_camera_target: Query<&RenderTarget>,
    q_csm: Query<&CascadeShadowMapping>,
    rs: Res<RenderState>,
) {
    let (_window, surface_state) = window.into_inner();

    let surface_texture = match surface_state.surface.get_current_texture() {
        Ok(st) => st,
        Err(status) => {
            bevy_log::error!("Failed to acquire surface texture: {:?}", status);
            return;
        }
    };

    // Copy the rendered scene to the surface so it shows through
    // iced's transparent center panel.
    for target in q_camera_target.iter() {
        if let TargetType::Texture(scene) = &target.target_type {
            let scene_size = scene.texture.size();
            let surface_size = surface_texture.texture.size();

            let copy_width = scene_size.width.min(surface_size.width);
            let copy_height = scene_size.height.min(surface_size.height);

            let mut encoder = rs.device.create_command_encoder(&Default::default());
            lentille_wgpu_utils::copy_texture2d_to_texture2d_no_mip(
                &mut encoder,
                scene.texture.texture(),
                &surface_texture.texture,
                Extent3d {
                    width: copy_width,
                    height: copy_height,
                    depth_or_array_layers: 1,
                },
            );
            rs.queue.submit(std::iter::once(encoder.finish()));
            break;
        }
    }

    let view = surface_texture.texture.create_view(&Default::default());
    // Take the one-shot focus flags so we can borrow ui_state immutably.
    let mut focus_rename = std::mem::take(&mut editor.focus_rename_input);
    let mut focus_prop = std::mem::take(&mut editor.focus_property_input);
    let messages = iced.update_and_draw(
        &editor.ui_state,
        &mut focus_rename,
        &mut focus_prop,
        &view,
    );
    editor.focus_rename_input = focus_rename;
    editor.focus_property_input = focus_prop;

    // Overlay CSM preview quads ON TOP of the iced UI.
    render_csm_previews(&mut preview, &editor.ui_state, &iced, &q_csm, &rs, &surface_texture);

    editor.pending_messages = messages;
    surface_texture.present();
}

fn render_csm_previews(
    preview: &mut PreviewState,
    ui_state: &crate::editor::data_types::EditorUiState,
    iced: &IcedRenderer,
    q_csm: &Query<&CascadeShadowMapping>,
    rs: &RenderState,
    surface_texture: &wgpu::SurfaceTexture,
) {
    let Some(csm) = q_csm.iter().next() else {
        return;
    };

    let layer_count = csm.layers.len();
    if layer_count == 0 {
        return;
    }

    let depth_size = csm.shadow_maps.size();
    let depth_w = depth_size.width;
    let depth_h = depth_size.height;

    ensure_csm_outputs(preview, &rs.device, layer_count, depth_w, depth_h);

    let viewport_w = iced.viewport.physical_size().width;
    let viewport_h = iced.viewport.physical_size().height;
    let scale = iced.viewport.scale_factor();
    let right_panel_w = (ui_state.right_panel_width * scale) as u32;
    let right_panel_x = viewport_w.saturating_sub(right_panel_w);

    let gap = (12.0 * scale) as u32;
    let inner_w = right_panel_w.saturating_sub(gap * 2);

    let cols = if inner_w >= 200 { 2 } else { 1 };
    let mut preview_size = if cols == 2 {
        (inner_w.saturating_sub(gap)) / 2
    } else {
        inner_w
    };
    preview_size = preview_size.min(256);

    let inspector_offset = ((ui_state.inspector_height + 60.0) * scale) as u32;
    let header_h = (40.0 * scale) as u32 + inspector_offset;

    let mut encoder = rs.device.create_command_encoder(&Default::default());

    for (i, layer) in csm.layers.iter().enumerate().take(preview.csm_outputs.len()) {
        let (_, ref rgba_view, _, _, _) = preview.csm_outputs[i];

        preview.depth_to_rgba.convert_to(
            &rs.device,
            &mut encoder,
            layer.view.as_ref(),
            rgba_view,
            depth_w,
            depth_h,
        );
    }

    let surface_view = surface_texture.texture.create_view(&Default::default());

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("csm_preview_blit"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&preview.preview_blit.pipeline);
        rpass.set_vertex_buffer(0, preview.preview_blit.vertex_buf.slice(..));

        for (i, &(_, _, ref bind_group, _, _)) in preview.csm_outputs.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let dst_x = right_panel_x + gap + (col as u32) * (preview_size + gap);
            let dst_y = header_h + (row as u32) * (preview_size + gap);

            if dst_x + preview_size > viewport_w || dst_y + preview_size > viewport_h {
                continue;
            }

            rpass.set_viewport(
                dst_x as f32,
                dst_y as f32,
                preview_size as f32,
                preview_size as f32,
                0.0,
                1.0,
            );

            rpass.set_bind_group(0, bind_group, &[]);
            rpass.draw(0..6, 0..1);
        }
    }

    rs.queue.submit(std::iter::once(encoder.finish()));
}

fn build_property_lines(
    editor: &mut EditorUiResource,
    q_camera: &Query<&Camera>,
    q_plight: &Query<&PointLight>,
    q_dlight: &Query<&ParallelLight>,
    q_named: &Query<(Entity, &Name, &Transform)>,
    q_world: &Query<&WorldTransform>,
) {
    let Some(selected) = editor.ui_state.selected_entity else {
        editor.ui_state.property_lines.clear();
        editor.ui_state.inspector_height = 0.0;
        return;
    };

    use PropertyFieldId::*;

    let mut fields: Vec<PropertyField> = Vec::new();

    // Helper to push a section header.
    fn push_header(fields: &mut Vec<PropertyField>, title: &str) {
        fields.push(PropertyField {
            label: format!("── {} ──", title),
            value: String::new(),
            draft: String::new(),
            field_id: PropertyFieldId::PositionX, // dummy — ignored for headers
            is_header: true,
            error: None,
        });
    }
    // Helper to push an editable field.
    fn push_field(fields: &mut Vec<PropertyField>, label: &str, value: String, field_id: PropertyFieldId) {
        fields.push(PropertyField {
            label: label.to_string(),
            draft: value.clone(),
            value,
            field_id,
            is_header: false,
            error: None,
        });
    }

    if let Ok((_, name, transform)) = q_named.get(selected) {
        // Name is shown as a header since it's edited via the tree double-click
        push_header(&mut fields, &format!("Name: {}", name));
        push_header(&mut fields, "Transform");
        push_field(&mut fields, "Position X", format!("{:.2}", transform.position.x), PositionX);
        push_field(&mut fields, "Position Y", format!("{:.2}", transform.position.y), PositionY);
        push_field(&mut fields, "Position Z", format!("{:.2}", transform.position.z), PositionZ);

        let euler = Euler::from(transform.rotation);
        let rx: Deg<f32> = Deg::from(euler.x);
        let ry: Deg<f32> = Deg::from(euler.y);
        let rz: Deg<f32> = Deg::from(euler.z);
        push_field(&mut fields, "Rotation X", format!("{:.1}", rx.0), RotationX);
        push_field(&mut fields, "Rotation Y", format!("{:.1}", ry.0), RotationY);
        push_field(&mut fields, "Rotation Z", format!("{:.1}", rz.0), RotationZ);

        push_field(&mut fields, "Scale X", format!("{:.2}", transform.scale.x), ScaleX);
        push_field(&mut fields, "Scale Y", format!("{:.2}", transform.scale.y), ScaleY);
        push_field(&mut fields, "Scale Z", format!("{:.2}", transform.scale.z), ScaleZ);
    }

    if let Ok(camera) = q_camera.get(selected) {
        push_header(&mut fields, "Camera");
        push_field(&mut fields, "FOV", format!("{:.1}", camera.fovy), CameraFov);
        push_field(&mut fields, "Near", format!("{:.3}", camera.znear), CameraNear);
        push_field(&mut fields, "Far", format!("{:.1}", camera.zfar), CameraFar);
    }

    if let Ok(plight) = q_plight.get(selected) {
        push_header(&mut fields, "Point Light");
        push_field(&mut fields, "Intensity", format!("{:.2}", plight.intensity), PointLightIntensity);
        push_field(
            &mut fields,
            "Color",
            format!("{:.2}, {:.2}, {:.2}", plight.color.r(), plight.color.g(), plight.color.b()),
            PointLightColor,
        );
    }

    if let Ok(dlight) = q_dlight.get(selected) {
        push_header(&mut fields, "Dir Light");
        push_field(&mut fields, "Intensity", format!("{:.2}", dlight.intensity), DirLightIntensity);
        push_field(
            &mut fields,
            "Color",
            format!("{:.2}, {:.2}, {:.2}", dlight.color.r(), dlight.color.g(), dlight.color.b()),
            DirLightColor,
        );
    }

    if let Ok(world) = q_world.get(selected) {
        push_header(&mut fields, "World");
        // WorldPos is read-only — mark as headers
        push_header(&mut fields, &format!("Pos X: {:.2}", world.position.x));
        push_header(&mut fields, &format!("Pos Y: {:.2}", world.position.y));
        push_header(&mut fields, &format!("Pos Z: {:.2}", world.position.z));
    }

    // Apply errors from the last commit.
    for (idx, err) in editor.property_errors.drain(..) {
        if let Some(field) = fields.get_mut(idx) {
            field.error = Some(err);
        }
    }

    // Estimate inspector logical height: ~22px per editable row + ~16px per header + padding
    let row_count = fields.iter().filter(|f| !f.is_header).count();
    let header_count = fields.iter().filter(|f| f.is_header).count();
    editor.ui_state.inspector_height = row_count as f32 * 22.0 + header_count as f32 * 16.0 + 16.0;
    editor.ui_state.property_lines = fields;
}

fn ensure_csm_outputs(
    preview: &mut PreviewState,
    device: &wgpu::Device,
    count: usize,
    width: u32,
    height: u32,
) {
    preview.csm_outputs.retain(|(_, _, _, w, h)| *w == width && *h == height);

    while preview.csm_outputs.len() < count {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("csm_preview_output"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        // Pre-create the bind group so the render loop doesn't allocate per frame.
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("csm_preview_bg"),
            layout: &preview.preview_blit.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&preview.preview_blit.sampler),
                },
            ],
        });
        preview.csm_outputs.push((tex, view, bind_group, width, height));
    }
}
