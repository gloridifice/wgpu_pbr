use bevy_app::Last;
use bevy_ecs::{prelude::*, schedule::ScheduleLabel, system::ScheduleSystem};

use crate::{
    FrameSets, RenderState,
    resource::{InitConfig, RENDER_RESOURCES_TO_ADD},
};

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
    fn init_render_resource_with_config<T: Resource + FromWorld>(
        &mut self,
        configs: impl Into<Vec<Box<dyn InitConfig>>>,
    ) -> &mut Self;
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

    fn init_render_resource<T: Resource + FromWorld>(&mut self) -> &mut Self {
        RENDER_RESOURCES_TO_ADD.lock().unwrap().insert::<T>();
        self
    }

    fn init_render_resource_with_config<T: Resource + FromWorld>(
        &mut self,
        configs: impl Into<Vec<Box<dyn InitConfig>>>,
    ) -> &mut Self {
        RENDER_RESOURCES_TO_ADD
            .lock()
            .unwrap()
            .insert_with_configs::<T>(configs.into());
        self
    }
}
