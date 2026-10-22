use crate::{
    asset::{GpuHandle, SceneAssetCache},
    material::pbr::UploadedPbrMaterial,
    prelude::*,
    transform::TransformUniform,
};
use bevy_ecs::prelude::*;

// Runtime-only. Cloning an entity must rebuild its independent object buffer.
#[derive(Component)]
#[component(clone_behavior = Ignore)]
pub struct MeshRenderer {
    pub mesh: GpuHandle<UploadedMesh>,
    pub materials: Vec<Arc<UploadedPbrMaterial>>,
    pub object_bind_group: Arc<BindGroup>,
    pub transform_buffer: Arc<TypedBuffer<TransformUniform>>,
}
impl MeshRenderer {
    pub fn new(
        mesh: GpuHandle<UploadedMesh>,
        materials: Vec<Arc<UploadedPbrMaterial>>,
        world: &World,
    ) -> Self {
        let device = &world.resource::<RenderState>().device;
        let buffer = TypedBuffer::new(
            device,
            Some("Object transform"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );
        let object_bind_group=device.create_bind_group(&bg_descriptor!(["Object"] [&world.resource::<ObjectBindGroupLayout>().0] 0: buffer.as_entire_binding();));
        Self {
            mesh,
            materials,
            object_bind_group: Arc::new(object_bind_group),
            transform_buffer: Arc::new(buffer),
        }
    }
    pub fn update_transform_buffer(&self, queue: &wgpu::Queue, uniform: TransformUniform) {
        self.transform_buffer.write(uniform, queue);
    }
    pub fn draw(&self, pass: &mut RenderPass, assets: &SceneAssetCache) {
        pass.set_bind_group(1, self.object_bind_group.as_ref(), &[]);
        self.draw_primitives(pass, assets);
    }
    pub fn draw_primitives(&self, pass: &mut RenderPass, assets: &SceneAssetCache) {
        let Some(mesh) = assets.mesh(self.mesh) else {
            return;
        };
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for p in &mesh.primitives {
            pass.draw_indexed(p.indices_start..p.indices_start + p.indices_num, 0, 0..1);
        }
    }
    pub fn draw_filtered(
        &self,
        pass: &mut RenderPass,
        assets: &SceneAssetCache,
        transparent: bool,
    ) {
        let Some(mesh) = assets.mesh(self.mesh) else {
            return;
        };
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(2, self.object_bind_group.as_ref(), &[]);
        for (p, material) in mesh.primitives.iter().zip(&self.materials) {
            if (material.alpha_mode == AlphaMode::Blend) != transparent {
                continue;
            }
            pass.set_bind_group(1, material.bind_group.as_ref(), &[]);
            pass.draw_indexed(p.indices_start..p.indices_start + p.indices_num, 0, 0..1);
        }
    }
}
