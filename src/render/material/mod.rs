use super::prelude::*;

pub mod buffer_material;
pub mod pbr;

pub trait UploadedMaterial {
    /// Return the material bind group
    fn get_bind_group(&self) -> &BindGroup;
    fn get_render_pipeline(&self) -> &RenderPipeline;
}
