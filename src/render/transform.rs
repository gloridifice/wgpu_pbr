use std::sync::Arc;

use bevy_ecs::{component::Component, entity::Entity, system::Resource, world::World};
use cgmath::{Deg, EuclideanSpace, Matrix4, Point3, Quaternion, Rotation3, SquareMatrix, Vector3};
use derive_builder::Builder;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, Device};

use crate::wgpu_init::bind_group_layout_entry_shader;

#[derive(Component, Builder, Clone, Debug)]
pub struct Transform {
    #[builder(default = None)]
    pub parent: Option<Entity>,
    #[builder(default = vec![])]
    pub children: Vec<Entity>,
    #[builder(default = Point3::<f32>::new(0.,0.,0.))]
    pub position: Point3<f32>,
    #[builder(default = Quaternion::<f32>::new(1., 0., 0., 0.))]
    pub rotation: Quaternion<f32>,
    #[builder(default = Vector3::<f32>::new(1.,1.,1.))]
    pub scale: Vector3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        TransformBuilder::default().build().unwrap()
    }
}

impl Transform {
    pub fn with_position(pos: Point3<f32>) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    pub fn local_matrix4x4(&self) -> Matrix4<f32> {
        let translation = Matrix4::from_translation(self.position.to_vec());
        let scale = Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);
        let rotation = Matrix4::from(self.rotation);
        let ret = translation * rotation * scale;
        ret
    }

    pub fn calculate_world_matrix4x4(&self, world: &World) -> Matrix4<f32> {
        let local_matrix = self.local_matrix4x4();
        if let Some(parent) = self.parent {
            return local_matrix
                * world
                    .get::<Transform>(parent)
                    .expect("Transform's parent entity don't have Transform component")
                    .calculate_world_matrix4x4(world);
        };
        return local_matrix;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TransformUniform {
    pub model: [[f32; 4]; 4],
}

impl TransformUniform {
    const ENTRIES: [BindGroupLayoutEntry; 1] = [bind_group_layout_entry_shader(
        0,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )];
    pub fn layout_desc() -> BindGroupLayoutDescriptor<'static> {
        BindGroupLayoutDescriptor {
            label: Some("Transform Bind Group Layout"),
            entries: &Self::ENTRIES,
        }
    }
}

unsafe impl bytemuck::Pod for TransformUniform {}
unsafe impl bytemuck::Zeroable for TransformUniform {}

#[derive(Resource, Clone, Debug)]
pub struct TransformBindGroupLayout(pub Arc<BindGroupLayout>);

impl TransformBindGroupLayout {
    pub fn new(device: &Device) -> Self {
        let transform_bind_group_layout =
            Arc::new(device.create_bind_group_layout(&TransformUniform::layout_desc()));
        Self(transform_bind_group_layout)
    }
}
