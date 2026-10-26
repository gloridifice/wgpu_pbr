use super::{
    codec::{SceneCodec, SceneOwned},
    controller::snapshot,
};
use anyhow::Result;
use bevy_ecs::{change_detection::Tick, component::ComponentId, prelude::*};

/// Tracks the serialized component set, not runtime/derived ECS state.
#[derive(Default)]
pub(super) struct SnapshotCache {
    last_check: Option<Tick>,
    membership: Vec<(Entity, Option<ComponentId>)>,
    text: Option<String>,
}

impl SnapshotCache {
    pub fn current(&mut self, world: &mut World, codec: &SceneCodec) -> Result<&str> {
        lentille_core::profile_scope!("SceneSnapshotCheck");
        // Establish a boundary even when called directly outside a schedule:
        // subsequent World mutations must not share the tick we just observed.
        let now = world.increment_change_tick();
        let last = self.last_check.unwrap_or(now);
        let mut changed =
            self.text.is_none() || now.get().wrapping_sub(last.get()) > Tick::MAX.get();
        let components = codec.persistent_component_ids(world);
        let mut membership = Vec::with_capacity(self.membership.len());
        let mut query = world.query_filtered::<EntityRef, With<SceneOwned>>();
        for entity in query.iter(world) {
            // Include owned entities without any serializable components as well.
            membership.push((entity.id(), None));
            for &id in &components {
                if let Some(ticks) = entity.get_change_ticks_by_id(id) {
                    membership.push((entity.id(), Some(id)));
                    changed |= ticks.is_changed(last, now);
                }
            }
        }
        membership.sort_unstable();
        // Detect removals/despawns without depending on a bounded event history.
        changed |= membership != self.membership;
        if changed {
            // On failure retain the old boundary so a later call retries.
            self.text = Some(snapshot(world, codec)?);
            self.membership = membership;
        }
        self.last_check = Some(now);
        Ok(self.text.as_deref().expect("successful snapshot cached"))
    }
}
