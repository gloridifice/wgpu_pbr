use bevy_ecs::prelude::Query;
use bevy_ecs::{component::Component, entity::Entity};
use cgmath::{ElementWise, Matrix3, Matrix4, Rotation, SquareMatrix, Vector3};
use derive_builder::Builder;

use crate::cgmath_ext::{Mat3, Mat4};
use crate::cgmath_ext::{Quat, QuatExt, Vec3, Vector3Ext, VectorExt};

#[allow(unused)]
#[derive(Component, Builder, Clone, Debug)]
#[require(WorldTransform)]
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

#[derive(Component, Clone)]
pub struct WorldTransform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct TransformUniform {
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 3],
}

unsafe impl bytemuck::Pod for TransformUniform {}
unsafe impl bytemuck::Zeroable for TransformUniform {}

impl Default for WorldTransform {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            rotation: Quat::identity(),
            scale: Vec3::zero(),
        }
    }
}

pub fn sys_update_world_transform(
    mut q_transform: Query<(Entity, &Transform, &mut WorldTransform)>,
) {
    let vec = q_transform
        .iter()
        .map(|(id, trans, _)| {
            let world_transform = cal_world_transform(trans, &q_transform);
            (id, world_transform)
        })
        .collect::<Vec<_>>();
    vec.into_iter().for_each(|(id, world_transform)| {
        let (_, _, mut to_modified) = q_transform.get_mut(id).unwrap();
        *to_modified = world_transform;
    });
}

pub fn cal_world_transform(
    transform: &Transform,
    query: &Query<(Entity, &Transform, &mut WorldTransform)>,
) -> WorldTransform {
    if let Some(parent_id) = transform.parent {
        if let Ok((_, p_trans, _)) = query.get(parent_id) {
            let parent_world_trans = cal_world_transform(p_trans, query);
            return WorldTransform {
                position: transform.position + parent_world_trans.position,
                rotation: parent_world_trans.rotation * transform.rotation,
                scale: parent_world_trans.scale.mul_element_wise(transform.scale),
            };
        }
    }
    WorldTransform {
        position: transform.position,
        rotation: transform.rotation,
        scale: transform.scale,
    }
}

impl Default for Transform {
    fn default() -> Self {
        TransformBuilder::default().build().unwrap()
    }
}

impl Transform {
    #[allow(unused)]
    pub fn with_position(pos: Vec3) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    #[allow(unused)]
    pub fn forward(&self) -> Vector3<f32> {
        let fwd = Vector3::new_z(-1.);
        self.rotation.rotate_vector(fwd)
    }
}

impl WorldTransform {
    pub fn get_uniform(&self) -> TransformUniform {
        let (model, normal) = self.model_normal_matrix();
        TransformUniform {
            model: model.into(),
            normal: [
                normal.x.with_w(0.).into(),
                normal.y.with_w(0.).into(),
                normal.z.with_w(0.).into(),
            ],
        }
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * Vector3::new_z(-1.)
    }

    #[allow(unused)]
    pub fn up(&self) -> Vec3 {
        self.rotation * Vector3::new_y(1.)
    }

    #[allow(unused)]
    pub fn left(&self) -> Vec3 {
        self.rotation * Vec3::new_x(-1.)
    }

    const MIN_SACLE: f32 = 0.0001;

    pub fn model_normal_matrix(&self) -> (Mat4, Mat3) {
        let translation = Matrix4::from_translation(self.position);
        let scale = Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);
        let rotation = Matrix4::from(self.rotation);
        #[rustfmt::skip]
        let scale_t = Matrix3::new(
            1. / self.scale.x.max(Self::MIN_SACLE), 0.0, 0.0,
            0.0, 1. / self.scale.y.max(Self::MIN_SACLE), 0.0,
            0.0, 0.0, 1. / self.scale.z.max(Self::MIN_SACLE)
        );
        let model_matrix = translation * rotation * scale;
        let normal_matrix = Matrix3::from_cols(
            rotation.x.truncate(),
            rotation.y.truncate(),
            rotation.z.truncate(),
        ) * scale_t;
        (model_matrix, normal_matrix)
    }

    pub fn view_matrix(&self) -> Mat4 {
        let translation = Mat4::from_translation(-self.position);
        let rotation = Matrix4::from(self.rotation).invert().unwrap();
        rotation * translation
    }
}

pub fn sys_update_children(mut q_transform: Query<(Entity, &mut Transform)>) {
    let child_parent = q_transform
        .iter()
        .filter_map(|(id, trans)| trans.parent.map(|parent| (id, parent)))
        .collect::<Vec<_>>();

    for (child, parent) in child_parent.into_iter() {
        if let Ok((_, mut trans)) = q_transform.get_mut(parent) {
            if !trans.children.contains(&child) {
                trans.children.push(child);
            }
        }
    }
}
