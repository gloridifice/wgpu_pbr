use crate::{
    material::{
        UploadedMaterial,
        pbr::{PBRMaterialOverride, UploadedPbrMaterial},
    },
    prelude::*,
    transform::TransformUniform,
};
use bevy_ecs::prelude::*;

#[derive(Component, Clone)]
pub struct MeshRenderer {
    pub mesh: Option<Arc<UploadedMesh>>,
    pub object_bind_group: Arc<BindGroup>,
    pub transform_buffer: Arc<TypedBuffer<TransformUniform>>,
}

impl MeshRenderer {
    pub fn new(mesh: Arc<UploadedMesh>, world: &World) -> Self {
        let device = &world.resource::<RenderState>().device;
        let layout = &world.resource::<ObjectBindGroupLayout>().0;

        let buffer = TypedBuffer::new(
            device,
            Some("transform buffer"),
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        );
        let object_bind_group = device.create_bind_group(&bg_descriptor!(
            ["Object Bind Group"] [layout]
            0: buffer.as_entire_binding();
        ));
        Self {
            mesh: Some(mesh),
            object_bind_group: Arc::new(object_bind_group),
            transform_buffer: Arc::new(buffer),
        }
    }

    pub fn update_transform_buffer(&self, queue: &wgpu::Queue, uniform: TransformUniform) {
        self.transform_buffer.write(uniform, queue);
    }

    /// Bind vertex buffer and index buffer, and set bind group of 1 (ObjectBindGroup)
    pub fn draw(&self, render_pass: &mut RenderPass) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };

        render_pass.set_bind_group(1, self.object_bind_group.as_ref(), &[]);
        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for primitive in mesh.primitives.iter() {
            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }

    fn check_opaque(
        primitive: &UploadedPrimitive,
        override_material: Option<&PBRMaterialOverride>,
    ) -> bool {
        primitive.material.as_ref().is_none_or(|mat| {
            mat.alpha_mode_or_default() == AlphaMode::Opaque
                && override_material.is_none_or(|ove| {
                    ove.material
                        .alpha_mode
                        .is_none_or(|am| am == AlphaMode::Opaque)
                })
        })
    }

    pub fn draw_opaque(
        &self,
        render_pass: &mut RenderPass,
        default_material: Arc<UploadedPbrMaterial>,
        override_material: Option<&PBRMaterialOverride>,
    ) {
        self.draw_filtered(
            render_pass,
            default_material,
            override_material,
            |primitive| MeshRenderer::check_opaque(primitive, override_material),
        );
    }

    pub fn draw_transparent(
        &self,
        render_pass: &mut RenderPass,
        default_material: Arc<UploadedPbrMaterial>,
        override_material: Option<&PBRMaterialOverride>,
    ) {
        self.draw_filtered(
            render_pass,
            default_material,
            override_material,
            |primitive| !MeshRenderer::check_opaque(primitive, override_material),
        );
    }

    fn draw_filtered(
        &self,
        render_pass: &mut RenderPass,
        default_material: Arc<UploadedPbrMaterial>,
        override_material: Option<&PBRMaterialOverride>,
        is_valid: impl Fn(&UploadedPrimitive) -> bool,
    ) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };

        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(2, self.object_bind_group.as_ref(), &[]);

        let mut last_material: Option<Arc<UploadedPbrMaterial>> = None;

        if let Some(ove) = override_material.and_then(|it| it.uploaded_material.as_ref()) {
            render_pass.set_bind_group(1, ove.bind_group.as_ref(), &[]);
        }
        for primitive in mesh.primitives.iter() {
            // Filter
            if !is_valid(primitive) {
                continue;
            }

            if override_material.is_none() {
                let material_instance = match primitive.uploaded_material.as_ref() {
                    Some(a) => a,
                    None => &default_material,
                };
                if last_material.is_none()
                    || Arc::ptr_eq(last_material.as_ref().unwrap(), material_instance)
                {
                    last_material = Some(Arc::clone(material_instance));
                    render_pass.set_bind_group(1, material_instance.get_bind_group(), &[]);
                }
            }

            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }

    pub fn draw_primitives(&self, render_pass: &mut RenderPass) {
        let Some(mesh) = self.mesh.as_ref() else {
            return;
        };

        render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for primitive in mesh.primitives.iter() {
            let start = primitive.indices_start;
            let num = primitive.indices_num;
            render_pass.draw_indexed(start..(start + num), 0, 0..1);
        }
    }
}
