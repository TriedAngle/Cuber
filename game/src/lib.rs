extern crate nalgebra as na;

pub mod input;
pub use input::Input;

#[derive(Debug, Clone, Copy)]
pub struct Transform { 
    pub position: na::Vector3<f32>,
    pub rotation: na::UnitQuaternion<f32>,
    pub scale: na::Vector3<f32>,
}

impl Transform { 
    pub fn identity() -> Self { 
        Self { 
            position: na::Vector3::zeros(),
            rotation: na::UnitQuaternion::identity(),
            scale: na::Vector3::new(1.0, 1.0, 1.0),
        }
    }

    pub fn position(&mut self, position: &na::Vector3<f32>) { 
        self.position = *position;
    }

    pub fn scale(&mut self, scale: f32) { 
        self.scale *= scale;
    }

    pub fn scale_nonuniform(&mut self, scale: &na::Vector3<f32>) {
        self.scale = *scale;
    }

    pub fn rotate_around(&mut self, axis: &na::Unit<na::Vector3<f32>>, angle: f32) { 
        let rotation = na::UnitQuaternion::from_axis_angle(&axis, angle.to_radians());
        self.rotation = rotation * self.rotation;
    }

    pub fn rotate_locally(&mut self, axis: &na::Unit::<na::Vector3<f32>>, angle: f32) { 
        let local_axis = self.rotation * axis;
        let rotation = na::UnitQuaternion::from_axis_angle(&local_axis, angle.to_radians());
        self.rotation = rotation * self.rotation;
    }

    pub fn to_homogeneous(&self) -> na::Matrix4<f32> { 
        let translation = na::Matrix4::new_translation(&self.position);
        let rotation = self.rotation.to_homogeneous();
        let scale = na::Matrix4::new_nonuniform_scaling(&self.scale);

        translation * rotation * scale
    }
}
