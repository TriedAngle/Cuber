use crate::Input;
use winit::keyboard::KeyCode;

pub struct Camera {
    pub position: na::Point3<f32>,
    pub rotation: na::UnitQuaternion<f32>,
    pub speed: f32,
    pub sensitivity: f32,
    pub fov: f32,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn new(
        position: na::Point3<f32>,
        rotation: na::UnitQuaternion<f32>,
        speed: f32,
        sensitivity: f32,
        fov: f32,
        aspect: f32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            position,
            rotation,
            speed,
            sensitivity,
            fov,
            aspect,
            znear,
            zfar,
        }
    }

    pub fn look_at(&mut self, target: na::Point3<f32>, up: &na::Vector3<f32>) {
        let direction = self.position - target;
        self.rotation = na::UnitQuaternion::face_towards(&direction, &up);
    }

    pub fn update_mouse(&mut self, dt: f32, input: &Input) {
        let (yaw, pitch) = self.calculate_rotation(input, dt);

        let local_x_axis = self.rotation * na::Vector3::x_axis();
        let local_y_axis = self.rotation * na::Vector3::y_axis();

        let combined_rotation = na::UnitQuaternion::from_axis_angle(&local_y_axis, yaw)
            * na::UnitQuaternion::from_axis_angle(&local_x_axis, pitch);

        self.rotation = combined_rotation * self.rotation;
    }

    pub fn update_keyboard(&mut self, dt: f32, input: &Input) {
        let roll = self.calculate_roll(input, dt);
        let forward = self.rotation * na::Vector3::z_axis();
        let roll_quat = na::UnitQuaternion::from_axis_angle(&forward, roll);
        let new_rotation = roll_quat * self.rotation;

        if self.rotation != new_rotation {
            self.rotation = new_rotation;
        }

        let forward = self.rotation * -*na::Vector3::z_axis();
        let right = self.rotation * *na::Vector3::x_axis();
        let up = self.rotation * -*na::Vector3::y_axis();

        let mut movement = na::Vector3::zeros();
        if input.pressing(KeyCode::KeyW) {
            movement += forward;
        }
        if input.pressing(KeyCode::KeyS) {
            movement -= forward;
        }
        if input.pressing(KeyCode::KeyD) {
            movement += right;
        }
        if input.pressing(KeyCode::KeyA) {
            movement -= right;
        }
        if input.pressing(KeyCode::Space) {
            movement += up;
        }
        if input.pressing(KeyCode::ShiftLeft) {
            movement -= up;
        }

        self.position += movement * self.speed * dt;

        // TODO: move this out
        if input.pressing(KeyCode::ControlLeft) {
            let scroll = input.scroll().y;
            if scroll != 0.0 {
                let speed_factor = if self.speed < 10.0 {
                    0.5
                } else if self.speed < 20.0 {
                    2.0
                } else if self.speed < 50.0 {
                    10.0
                } else {
                    30.0
                };

                self.speed = (self.speed + scroll * speed_factor).max(0.1).min(1000.0);
                log::trace!("New speed: {}", self.speed);
            }
        }

        if input.pressing(KeyCode::KeyC) {
            let scroll = input.scroll().y;
            if scroll != 0.0 {
                let zoom_speed = 2.0;
                self.fov = (self.fov - scroll * zoom_speed).max(10.0).min(120.0);
                log::trace!("New FOV: {}", self.fov);
            }
        }
    }

    fn calculate_rotation(&self, input: &Input, dt: f32) -> (f32, f32) {
        let delta = input.cursor_move();
        let yaw = -delta.x * self.sensitivity * dt;
        let pitch = -delta.y * self.sensitivity * dt;
        (yaw, pitch)
    }

    fn calculate_roll(&self, input: &Input, dt: f32) -> f32 {
        let mut roll = 0.0;

        if input.pressing(KeyCode::KeyQ) {
            roll -= self.speed * 0.1 * dt;
        }
        if input.pressing(KeyCode::KeyE) {
            roll += self.speed * 0.1 * dt;
        }
        roll
    }

    pub fn view_matrix(&self) -> na::Matrix4<f32> {
        let inverse_rotation = self
            .rotation
            .inverse()
            .to_rotation_matrix()
            .to_homogeneous();
        let inverse_translation = na::Translation3::from(-self.position.coords).to_homogeneous();
        inverse_rotation * inverse_translation
    }

    pub fn projection_matrix(&self) -> na::Matrix4<f32> {
        na::Perspective3::new(self.aspect, self.fov.to_radians(), self.znear, self.zfar)
            .to_homogeneous()
    }

    pub fn view_projection_matrix(&self) -> na::Matrix4<f32> {
        let view = self.view_matrix();
        let projection = self.projection_matrix();

        projection * view
    }
}
