use bevy_app::Last;
use bevy_ecs::{prelude::*, system::ScheduleSystem};

use crate::{
    FrameSets,
    graph::InsertConfig,
    resource::RENDER_RESOURCES_TO_ADD,
    stage::{RenderContext, RenderStageManager},
};

pub trait AppExt {
    fn configure_render_stage<Stage: 'static>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self;

    fn add_frame_system<
        Stage: 'static,
        M,
        S: IntoSystem<InMut<'static, RenderContext>, (), M> + 'static,
    >(
        &mut self,
        system: S,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self;

    fn add_render_system_in_frame_set<M>(
        &mut self,
        set: FrameSets,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;

    fn init_render_resource<T: Resource + FromWorld>(&mut self) -> &mut Self;

    fn init_render_resource_with_config<T: Resource + FromWorld>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self;
}

impl AppExt for bevy_app::App {
    fn init_render_resource<T: Resource + FromWorld>(&mut self) -> &mut Self {
        RENDER_RESOURCES_TO_ADD.lock().unwrap().insert::<T>();
        self
    }

    fn init_render_resource_with_config<T: Resource + FromWorld>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self {
        RENDER_RESOURCES_TO_ADD
            .lock()
            .unwrap()
            .insert_with_configs::<T>(configs);
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

    fn configure_render_stage<Stage: 'static>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self {
        let mut render_stage_manager = self
            .world_mut()
            .get_resource_or_init::<RenderStageManager>();
        render_stage_manager.try_init_stage::<Stage>();
        render_stage_manager
            .stages
            .configure_node::<Stage>(configs.into());
        self
    }

    fn add_frame_system<
        Stage: 'static,
        M,
        S: IntoSystem<InMut<'static, RenderContext>, (), M> + 'static,
    >(
        &mut self,
        system: S,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self {
        let id = self.register_system(system);
        let mut render_stage_manager = self
            .world_mut()
            .get_resource_or_init::<RenderStageManager>();
        render_stage_manager.insert_system::<Stage, S>(id, configs);
        self
    }
}
