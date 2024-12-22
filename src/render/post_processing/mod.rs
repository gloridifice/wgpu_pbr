use std::sync::Arc;

use bevy_ecs::prelude::*;
use wgpu::{BindGroup, BindGroupLayout, PipelineLayout, PipelineLayoutDescriptor, RenderPipeline};

use crate::{bg_layout_descriptor, RenderState};


#[derive(Resource)]
pub struct PostProcessingPipeline{
    pub layout: PipelineLayout,
    pub pipeline: Arc<RenderPipeline>,
    pub bind_group_layout: Arc<BindGroupLayout>,
    pub bind_group: Arc<BindGroup>,
}

impl FromWorld for PostProcessingPipeline {
    fn from_world(world: &mut World) -> Self {
        todo!();
        let render_state = world.resource::<RenderState>();
        let desc = PipelineLayoutDescriptor{
            label: Some("Post Processing"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        };
    }
}

