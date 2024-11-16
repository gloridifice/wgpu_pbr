use std::sync::Arc;

use bevy_ecs::{component::Component, entity::Entity, system::Resource, world::World};
use cgmath::{Matrix3, Matrix4, Quaternion, Rotation, Vector3};
use derive_builder::Builder;
use wgpu::{BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, Device};

use crate::{
    math_type::{Quat, QuatExt, Vec3, Vector3Ext, VectorExt},
    wgpu_init::bind_group_layout_entry_shader,
};

#[derive(Component, Builder, Clone, Debug)]
pub struct Transform {
    #[builder(default = None)]
    pub parent: Option<Entity>,
    #[builder(default = vec![])]
    pub children: Vec<Entity>,
    #[builder(default = Vec3::zero())]
    pub position: Vec3,
    #[builder(default = Quat::identity())]
    pub rotation: Quat,
    #[builder(default = Vec3::one())]
    pub scale: Vec3,
}

pub struct WorldTransform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        TransformBuilder::default().build().unwrap()
    }
}

impl Transform {
    pub fn with_position(pos: Vec3) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    pub fn forward(&self) -> Vector3<f32> {
        let fwd = Vector3::new_z(-1.);
        self.rotation.rotate_vector(fwd)
    }

    pub fn local_matrix(&self) -> (Matrix4<f32>, Quaternion<f32>) {
        let translation = Matrix4::from_translation(self.position);
        let scale = Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);
        let rotation = Matrix4::from(self.rotation);
        let ret = translation * rotation * scale;
        (ret, self.rotation)
    }

    pub fn calculate_world_matrix(&self, world: &World) -> (Matrix4<f32>, Quaternion<f32>) {
        let local_matrix = self.local_matrix();
        if let Some(parent) = self.parent {
            let par = world
                .get::<Transform>(parent)
                .expect("Transform's parent entity don't have Transform component")
                .calculate_world_matrix(world);
            return (local_matrix.0 * par.0, local_matrix.1 * par.1);
        };
        return local_matrix;
    }

    pub fn get_uniform(&self, world: &World) -> TransformUniform {
        let (model, rotation) = self.calculate_world_matrix(world);
        TransformUniform {
            model: model.into(),
            rotation: Matrix3::from(rotation).into(),
            padding: [0.; 3],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TransformUniform {
    pub model: [[f32; 4]; 4],
    pub rotation: [[f32; 3]; 3],
    pub padding: [f32; 3],
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
