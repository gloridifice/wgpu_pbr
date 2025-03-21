use wgpu::RenderPipelineDescriptor;

pub mod cube;

pub struct RenderPipelineDescriptionBuilder<'a> {
    pub desc: RenderPipelineDescriptor<'a>,
}

impl RenderPipelineDescriptionBuilder<'_> {}
