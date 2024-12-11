extern crate nalgebra as na;

pub mod brick;
pub mod input;
pub mod worldgen;

pub use input::Input;
use std::{
    collections::HashMap,
    time::{self, Duration},
};

#[derive(Copy, Clone, Debug)]
pub struct RawPtr(*mut ());
impl RawPtr {
    pub fn new(ptr: *mut ()) -> Self {
        Self(ptr)
    }

    pub fn get(&self) -> *mut () {
        self.0
    }
}
unsafe impl Send for RawPtr {}
unsafe impl Sync for RawPtr {}

#[derive(Debug)]
pub struct Diagnostics {
    pub sys_times: HashMap<String, time::SystemTime>,
    pub timings: HashMap<String, time::Duration>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            sys_times: HashMap::new(),
            timings: HashMap::new(),
        }
    }

    pub fn start(&mut self, name: &str) {
        let now = time::SystemTime::now();
        self.sys_times.insert(name.to_owned(), now);
    }

    pub fn stop(&mut self, name: &str) {
        let start = self
            .sys_times
            .get(name)
            .map(|t| *t)
            .unwrap_or(time::SystemTime::now());
        let time = start.elapsed().unwrap();
        self.timings.insert(name.to_owned(), time);
    }

    pub fn insert(&mut self, name: &str, time: time::Duration) {
        self.timings.insert(name.to_owned(), time);
    }

    pub fn time(&self, name: &str) -> time::Duration {
        self.timings.get(name).map(|t| *t).unwrap_or(Duration::ZERO)
    }

    pub fn time_millis(&self, name: &str) -> f64 {
        self.time(name).as_secs_f64() * 1000.0
    }
}

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

    pub fn rotate_locally(&mut self, axis: &na::Unit<na::Vector3<f32>>, angle: f32) {
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
