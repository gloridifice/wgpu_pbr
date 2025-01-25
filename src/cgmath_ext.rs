use cgmath::Point3;
use egui::Color32;

pub type Vec2 = cgmath::Vector2<f32>;
pub type Vec3 = cgmath::Vector3<f32>;
pub type Vec4 = cgmath::Vector4<f32>;
pub type Quat = cgmath::Quaternion<f32>;
pub type Mat4 = cgmath::Matrix4<f32>;
pub type Mat3 = cgmath::Matrix3<f32>;

#[allow(unused)]
pub trait VectorExt: Sized {
    fn zero() -> Self {
        Self::new_unit(0.)
    }
    fn one() -> Self {
        Self::new_unit(1.)
    }
    fn new_unit(v: f32) -> Self;
    fn new_x(x: f32) -> Self;
    fn new_y(y: f32) -> Self;
}

#[allow(unused)]
pub trait Vector3Ext {
    fn new_z(z: f32) -> Self;
    fn with_w(&self, w: f32) -> Vec4;
    fn into_point(&self) -> Point3<f32>;
}

#[allow(unused)]
pub trait Vector4Ext {
    fn new_w(w: f32) -> Self;
    fn new_z(z: f32) -> Self;
    fn to_color32(&self) -> Color32;
    fn from_color32(color: &Color32) -> Self;
}

impl VectorExt for Vec2 {
    fn new_unit(v: f32) -> Self {
        Vec2::new(v, v)
    }

    fn new_x(x: f32) -> Self {
        Vec2::new(x, 0.)
    }

    fn new_y(y: f32) -> Self {
        Vec2::new(0., y)
    }
}

impl VectorExt for Vec3 {
    fn new_unit(v: f32) -> Self {
        Self { x: v, y: v, z: v }
    }

    fn new_x(x: f32) -> Self {
        Self { x, y: 0., z: 0. }
    }

    fn new_y(y: f32) -> Self {
        Self { x: 0., y, z: 0. }
    }
}
impl Vector3Ext for Vec3 {
    fn new_z(z: f32) -> Self {
        Self { x: 0., y: 0., z }
    }

    fn into_point(&self) -> Point3<f32> {
        Point3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    fn with_w(&self, w: f32) -> Vec4 {
        Vec4::new(self.x, self.y, self.z, w)
    }
}

impl VectorExt for Vec4 {
    fn new_unit(v: f32) -> Self {
        Self::new(v, v, v, v)
    }

    fn new_x(x: f32) -> Self {
        Self::new(x, 0., 0., 0.)
    }

    fn new_y(y: f32) -> Self {
        Self::new(0., y, 0., 0.)
    }
}

impl Vector4Ext for Vec4 {
    fn new_z(z: f32) -> Self {
        Self::new(0., 0., z, 0.)
    }

    fn new_w(w: f32) -> Self {
        Self::new(0., 0., 0., w)
    }

    fn to_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            (self.x * 256.) as u8,
            (self.y * 256.) as u8,
            (self.z * 256.) as u8,
            (self.w * 256.) as u8,
        )
    }

    fn from_color32(color: &Color32) -> Self {
        Self {
            x: (color.r() as f32) / 256.,
            y: (color.g() as f32) / 256.,
            z: (color.b() as f32) / 256.,
            w: (color.a() as f32) / 256.,
        }
    }
}

pub trait QuatExt {
    fn identity() -> Self;
}

impl QuatExt for Quat {
    fn identity() -> Self {
        Quat::new(1., 0., 0., 0.)
    }
}
