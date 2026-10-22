use bevy_app::{Plugin, PostUpdate};
use bevy_ecs::prelude::*;
use bevy_reflect::Reflect;
use cgmath::{ElementWise, Matrix3, Matrix4, Rotation, SquareMatrix, Vector3};
use derive_builder::Builder;
use lentille_wgpu_utils::impl_pod_zeroable;

use lentille_math::*;

use crate::{RenderState, prelude::MeshRenderer};

pub(crate) struct TransformPlugin;

#[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone, Copy)]
pub struct TransformPostUpdateSet;

impl Plugin for TransformPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            PostUpdate,
            (
                sys_update_children,
                sys_update_world_transform,
                sys_update_transform_buffers,
            )
                .in_set(TransformPostUpdateSet)
                .chain(),
        );
    }
}

#[allow(unused)]
#[derive(Component, Builder, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
#[require(WorldTransform)]
pub struct Transform {
    #[builder(default = None)]
    #[entities]
    pub parent: Option<Entity>,
    #[builder(default = vec![])]
    #[reflect(ignore)]
    pub children: Vec<Entity>,
    #[builder(default = Vec3::zero())]
    #[reflect(remote = ReflectVec3)]
    pub position: Vec3,
    #[builder(default = Quat::identity())]
    #[reflect(remote = ReflectQuat)]
    pub rotation: Quat,
    #[builder(default = Vec3::one())]
    #[reflect(remote = ReflectVec3)]
    pub scale: Vec3,
}

#[derive(Component, Clone, Debug, PartialEq)]
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

impl_pod_zeroable!(TransformUniform);

impl Default for WorldTransform {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            rotation: Quat::identity(),
            scale: Vec3::one(),
        }
    }
}

pub fn sys_update_world_transform(
    mut q_transform: Query<(Entity, &Transform, &mut WorldTransform)>,
) {
    let local = q_transform
        .iter()
        .map(|(id, trans, _)| (id, trans.clone()))
        .collect();
    let Ok(worlds) = compute_world_transforms(&local) else {
        return;
    };
    worlds.into_iter().for_each(|(id, world_transform)| {
        let (_, _, mut to_modified) = q_transform.get_mut(id).unwrap();
        if *to_modified != world_transform {
            *to_modified = world_transform;
        }
    });
}

fn sys_update_transform_buffers(world: &mut World) {
    world.resource_scope(|world, render_state: Mut<RenderState>| {
        let mut query =
            world.query_filtered::<(&WorldTransform, &MeshRenderer), Or<(Changed<WorldTransform>, Added<MeshRenderer>)>>();
        for (world_trans, mesh_renderer) in query.iter(world) {
            mesh_renderer.update_transform_buffer(&render_state.queue, world_trans.get_uniform());
        }
    });
}

/// Resolve each parent before its children. Used by both candidate preparation
/// and frame updates, with no recursion or stale previous-frame parent values.
pub fn compute_world_transforms(
    local: &std::collections::HashMap<Entity, Transform>,
) -> anyhow::Result<std::collections::HashMap<Entity, WorldTransform>> {
    use std::collections::{HashMap, HashSet};
    let mut resolved: HashMap<Entity, WorldTransform> = HashMap::new();
    for &entity in local.keys() {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut cursor = Some(entity);
        while let Some(id) = cursor {
            if resolved.contains_key(&id) {
                break;
            }
            anyhow::ensure!(seen.insert(id), "Transform parent cycle");
            let transform = local
                .get(&id)
                .ok_or_else(|| anyhow::anyhow!("Missing parent transform"))?;
            chain.push(id);
            cursor = transform.parent;
        }
        for id in chain.into_iter().rev() {
            let transform = &local[&id];
            let world = if let Some(parent) = transform.parent {
                let parent = &resolved[&parent];
                WorldTransform {
                    position: parent.position
                        + parent
                            .rotation
                            .rotate_vector(parent.scale.mul_element_wise(transform.position)),
                    rotation: parent.rotation * transform.rotation,
                    scale: parent.scale.mul_element_wise(transform.scale),
                }
            } else {
                WorldTransform {
                    position: transform.position,
                    rotation: transform.rotation,
                    scale: transform.scale,
                }
            };
            resolved.insert(id, world);
        }
    }
    Ok(resolved)
}

impl Default for Transform {
    fn default() -> Self {
        TransformBuilder::default().build().unwrap()
    }
}

impl Transform {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(unused)]
    pub fn from_position(pos: Vec3) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    pub fn position(mut self, value: Vec3) -> Self {
        self.position = value;
        self
    }

    pub fn rotation(mut self, value: Quat) -> Self {
        self.rotation = value;
        self
    }

    pub fn scale_vec3(mut self, value: Vec3) -> Self {
        self.scale = value;
        self
    }

    pub fn scale(mut self, value: f32) -> Self {
        self.scale = Vec3::new_unit(value);
        self
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

    const MIN_SCALE: f32 = 0.0001;

    pub fn model_normal_matrix(&self) -> (Mat4, Mat3) {
        let translation = Matrix4::from_translation(self.position);
        let scale = Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);
        let rotation = Matrix4::from(self.rotation);
        #[rustfmt::skip]
        let scale_t = Matrix3::new(
            1. / self.scale.x.max(Self::MIN_SCALE), 0.0, 0.0,
            0.0, 1. / self.scale.y.max(Self::MIN_SCALE), 0.0,
            0.0, 0.0, 1. / self.scale.z.max(Self::MIN_SCALE)
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

    for (_, mut transform) in q_transform.iter_mut() {
        transform.bypass_change_detection().children.clear();
    }
    for (child, parent) in child_parent.into_iter() {
        if let Ok((_, mut trans)) = q_transform.get_mut(parent) {
            trans.bypass_change_detection().children.push(child);
        }
    }
}
