//! ECS components and resources for the ray-picking system.
//!
//! # Architecture
//! - `Pickable` marker + `PickingID` (auto-assigned u32) on each entity.
//! - `PickingIDMap` resource keeps the bidirectional mapping.
//! - `PickRequest` is a one-shot resource created on mouse click.
//! - `PickReadback` drives the GPU → CPU readback across frames.
//! - `LastPickResult` stores the result for downstream systems.
//! - `SelectedObjects` maintains the list of currently-selected entities.

use std::sync::{Arc, Mutex};

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use lentille_math::Vec2;

// ===== Components =====

/// Marker component: this entity (and its subtree) participates in ray-picking.
#[derive(Component, Clone, Debug)]
pub struct Pickable;

/// Auto-assigned unique identifier for GPU-CPU entity mapping.
/// Assigned by `sys_assign_picking_ids` when a `Pickable` entity is first seen.
#[derive(Component, Clone, Copy, Debug)]
pub struct PickingID(pub u32);

/// Marker for the currently-selected entity.
/// Added/removed by `sys_handle_pick_events`.
#[derive(Component, Clone, Debug)]
pub struct Selected;

// ===== Resources =====

/// Bidirectional map between picking IDs and ECS entities.
///
/// ID `0` is reserved and never handed out: the picking pass clears its target
/// to zero, so `0` means "nothing was hit".
#[derive(Resource, Debug)]
pub struct PickingIDMap {
    pub id_to_entity: HashMap<u32, Entity>,
    pub entity_to_id: HashMap<Entity, u32>,
    pub next_id: u32,
}

impl Default for PickingIDMap {
    fn default() -> Self {
        Self {
            id_to_entity: HashMap::new(),
            entity_to_id: HashMap::new(),
            next_id: 1,
        }
    }
}

impl PickingIDMap {
    pub fn assign(&mut self, entity: Entity) -> u32 {
        if let Some(existing) = self.entity_to_id.get(&entity) {
            return *existing;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.id_to_entity.insert(id, entity);
        self.entity_to_id.insert(entity, id);
        id
    }

    pub fn release(&mut self, entity: Entity) {
        if let Some(id) = self.entity_to_id.remove(&entity) {
            self.id_to_entity.remove(&id);
        }
    }

    pub fn lookup(&self, id: u32) -> Option<Entity> {
        self.id_to_entity.get(&id).copied()
    }
}

/// One-shot request to perform a GPU pick at a given screen position.
/// Consumed by the picking render pass.
#[derive(Resource, Clone, Debug)]
pub struct PickRequest {
    /// Pixel position within the render target (top-left origin).
    pub screen_pos: Vec2,
    /// Render target resolution at the time of the click.
    pub target_size: Vec2,
    /// Which camera's render target to use.
    pub camera_entity: Entity,
}

/// Progress of the GPU → CPU readback of the picking result.
///
/// The pick pass is recorded during the render (`Last`), so the staging buffer
/// can only be mapped on a *later* frame.  This resource tracks that hand-off.
#[derive(Resource, Default)]
pub enum PickReadback {
    /// Nothing in flight.
    #[default]
    Idle,
    /// The pick pass and its copy command were submitted; map on the next frame.
    AwaitingMap,
    /// `map_async` was issued; the callback fills the slot when the GPU is done.
    Mapping(Arc<Mutex<Option<Result<(), wgpu::BufferAsyncError>>>>),
}

impl PickReadback {
    pub fn is_idle(&self) -> bool {
        matches!(self, PickReadback::Idle)
    }
}

/// The result of the most recent pick, updated each time a `PickRequest` is
/// processed (with one frame of GPU-readback latency).
#[derive(Resource, Clone, Debug)]
pub struct LastPickResult {
    pub entity: Option<Entity>,
    pub screen_pos: Vec2,
}

impl Default for LastPickResult {
    fn default() -> Self {
        Self {
            entity: None,
            screen_pos: Vec2::new(0.0, 0.0),
        }
    }
}

/// Ordered list of currently-selected entities (most recent last).
#[derive(Resource, Default, Clone, Debug)]
pub struct SelectedObjects {
    pub entities: Vec<Entity>,
}

impl SelectedObjects {
    pub fn is_selected(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    pub fn select(&mut self, entity: Entity) {
        self.entities.retain(|e| *e != entity);
        self.entities.push(entity);
    }

    pub fn deselect(&mut self, entity: Entity) {
        self.entities.retain(|e| *e != entity);
    }

    pub fn clear(&mut self) {
        self.entities.clear();
    }

    pub fn toggle(&mut self, entity: Entity) {
        if self.is_selected(entity) {
            self.deselect(entity);
        } else {
            self.select(entity);
        }
    }

    pub fn set_single(&mut self, entity: Option<Entity>) {
        self.clear();
        if let Some(e) = entity {
            self.entities.push(e);
        }
    }
}

// ===== Events =====

/// Fired when a pick completes (with one frame of GPU readback latency).
#[derive(Event, Clone, Debug)]
pub struct PickEvent {
    pub entity: Option<Entity>,
    pub screen_position: Vec2,
}

impl PickEvent {
    pub fn new(entity: Option<Entity>, screen_position: Vec2) -> Self {
        Self {
            entity,
            screen_position,
        }
    }
}
