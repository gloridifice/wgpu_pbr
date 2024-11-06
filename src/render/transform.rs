use bevy_ecs::{component::Component, entity::Entity, world::World};
use cgmath::{Deg, EuclideanSpace, Matrix4, Point3, Quaternion, Rotation3, SquareMatrix, Vector3};
use derive_builder::Builder;

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
