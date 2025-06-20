use std::sync::{LazyLock, Mutex};

use bevy_app::Last;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel, system::ScheduleSystem};

use crate::{FrameSets, RenderState, resource::ResourceGraph};

pub(super) static RENDER_RESOURCES_TO_ADD: LazyLock<Mutex<ResourceGraph>> =
    LazyLock::new(|| Mutex::new(ResourceGraph::new()));

pub trait AppExt {
    fn add_render_system<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    fn add_render_system_with_custom_schedule<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    fn add_render_system_in_frame_set<M>(
        &mut self,
        set: FrameSets,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    fn init_render_resource<T: Resource + FromWorld>(&mut self) -> &mut Self;
}

impl AppExt for bevy_app::App {
    fn add_render_system<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.main_mut()
            .add_systems(Last, systems.run_if(resource_exists::<RenderState>));
        self
    }

    fn add_render_system_with_custom_schedule<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.main_mut()
            .add_systems(schedule, systems.run_if(resource_exists::<RenderState>));
        self
    }

    fn add_render_system_in_frame_set<M>(
        &mut self,
        set: FrameSets,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.main_mut().add_systems(Last, systems.in_set(set));
        self
    }

    fn init_render_resource<T: Resource + FromWorld>(&mut self, stage: ResStage) -> &mut Self {
        RENDER_RESOURCES_TO_ADD
            .lock()
            .unwrap()
            .entry(stage)
            .or_insert(Default::default())
            .push(Box::new(|world: &mut World| {
                world.init_resource::<T>();
            }));
        self
    }
}
