use crate::render::prelude::*;
use bevy_ecs::prelude::*;

#[derive(Component, Clone)]
#[require(Transform)]
pub struct PointLight {
    pub color: Vec4,
    pub intensity: f32,
    pub distance: Option<f32>,
    pub decay: f32,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy)]
pub struct RawPointLight {
    pub color: [f32; 4],
    pub position: [f32; 4],
    pub intensity: f32,
    pub distance: f32,
    pub decay: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: Vec4::one(),
            intensity: 1.0,
            distance: None,
            decay: 1.0,
        }
    }
}

impl PointLight {
    pub fn raw(&self, transform: &WorldTransform) -> RawPointLight {
        let pos = transform.position;
        RawPointLight {
            color: self.color.into(),
            intensity: self.intensity,
            distance: self
                .distance
                .unwrap_or((self.intensity * 256.0 / self.decay).sqrt()),
            decay: self.decay,
            position: [pos.x, pos.y, pos.z, 1.0],
        }
    }
}
