use bevy_app::Last;
use bevy_ecs::{prelude::*, system::ScheduleSystem};

use crate::{
    FrameSets,
    graph::InsertConfig,
    resource::RENDER_RESOURCES_TO_ADD,
    stage::{RenderContext, RenderStage, RenderStageConfig, RenderStageManager},
};

pub trait AppExt {
    fn configure_render_stage<Stage: 'static>(
        &mut self,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self;

    fn add_render_system<T, M, S>(
        &mut self,
        system: S,
        stage_config: impl Into<RenderStageConfig<T>>,
    ) -> &mut Self
    where
        T: RenderStage + 'static,
        S: IntoSystem<InMut<'static, RenderContext>, (), M> + 'static;

    fn add_render_system_with_config<Stage, M, S>(
        &mut self,
        system: S,
        configs: impl Into<Vec<InsertConfig>>,
    ) -> &mut Self
    where
        Stage: 'static,
        S: IntoSystem<InMut<'static, RenderContext>, (), M> + 'static;

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

    fn add_render_system_with_config<
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

    fn add_render_system<T, M, S>(
        &mut self,
        system: S,
        stage_config: impl Into<RenderStageConfig<T>>,
    ) -> &mut Self
    where
        T: RenderStage + 'static,
        S: IntoSystem<InMut<'static, RenderContext>, (), M> + 'static,
    {
        self.add_render_system_with_config::<T, _, _>(system, stage_config.into().configs);
        self
    }
}
