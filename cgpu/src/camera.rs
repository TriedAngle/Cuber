use game::input::Input;
use winit::keyboard::KeyCode;

pub struct Camera {
    position: na::Point3<f32>,
    rotation: na::UnitQuaternion<f32>,
    speed: f32,
    sensitivity: f32,
    fov: f32,
    aspect: f32,
    znear: f32,
    zfar: f32,
    updated: bool,
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
            updated: false,
        }
    }

    pub fn look_at(&mut self, target: na::Point3<f32>, up: &na::Vector3<f32>) {
        let direction = self.position - target;

        self.rotation = na::UnitQuaternion::face_towards(&direction, &up);
    }

    pub fn update_mouse(&mut self, _dt: f32, input: &Input) {
        let (yaw, pitch) = self.calculate_rotation(input);

        let local_x_axis = self.rotation * na::Vector3::x_axis();
        let local_y_axis = self.rotation * na::Vector3::y_axis();

        let yaw_quat = na::UnitQuaternion::from_axis_angle(&local_y_axis, yaw);
        let pitch_quat = na::UnitQuaternion::from_axis_angle(&local_x_axis, pitch);

        let new_rotation = yaw_quat * pitch_quat * self.rotation;
        if self.rotation != new_rotation {
            self.updated = true;
            self.rotation = new_rotation;
        }
    }

    pub fn update_keyboard(&mut self, dt: f32, input: &Input) {
        let roll = self.calculate_roll(input, dt);
        let local_z_axis = self.rotation * na::Vector3::z_axis();
        let roll_quat = na::UnitQuaternion::from_axis_angle(&local_z_axis, roll);
        let new_rotation = roll_quat * self.rotation;

        if self.rotation != new_rotation {
            self.rotation = new_rotation;
            self.updated = true;
        }

        let forward = self.rotation * -*na::Vector3::z_axis();
        let right = self.rotation * *na::Vector3::x_axis();
        let up = self.rotation * *na::Vector3::y_axis();

        if input.pressing(KeyCode::KeyW) {
            self.position += forward * self.speed * dt;
            self.updated = true;
        }
        if input.pressing(KeyCode::KeyS) {
            self.position -= forward * self.speed * dt;
            self.updated = true;
        }

        if input.pressing(KeyCode::KeyD) {
            self.position += right * self.speed * dt;
            self.updated = true;
        }
        if input.pressing(KeyCode::KeyA) {
            self.position -= right * self.speed * dt;
            self.updated = true;
        }

        if input.pressing(KeyCode::Space) {
            self.position += up * self.speed * dt;
            self.updated = true;
        }
        if input.pressing(KeyCode::ShiftLeft) {
            self.position -= up * self.speed * dt;
            self.updated = true;
        }
    }

    fn calculate_rotation(&self, input: &Input) -> (f32, f32) {
        let delta = input.cursor_move();
        let yaw = -delta.x * self.sensitivity;
        let pitch = -delta.y * self.sensitivity;
        (yaw, pitch)
    }

    fn calculate_roll(&self, input: &Input, dt: f32) -> f32 {
        let mut roll = 0.0;

        if input.pressing(KeyCode::KeyQ) {
            roll += self.speed * 0.5 * dt;
        }
        if input.pressing(KeyCode::KeyE) {
            roll -= self.speed * 0.5 * dt;
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

    pub fn updated(&self) -> bool {
        self.updated
    }

    pub fn reset_update(&mut self) {
        self.updated = false;
    }
}
