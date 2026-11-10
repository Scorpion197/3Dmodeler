use glam::{Mat4, Vec3};

pub struct Camera {
    pub target: Vec3,
    pub distance: f32,
    yaw: f32,
    pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 8.0,
            yaw: 0.65,
            pitch: 0.4,
        }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        self.target
            + self.distance
                * Vec3::new(
                    self.yaw.sin() * self.pitch.cos(),
                    self.pitch.sin(),
                    self.yaw.cos() * self.pitch.cos(),
                )
    }

    pub fn matrix(&self, aspect: f32) -> Mat4 {
        // WebGPU uses a depth range of 0..1.
        let projection = Mat4::perspective_rh(45_f32.to_radians(), aspect, 0.05, 500.0);

        let view = Mat4::look_at_rh(self.eye(), self.target, Vec3::Y);

        projection * view
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.008;
        self.pitch = (self.pitch + dy * 0.008).clamp(-1.5, 1.5);
    }

    pub fn zoom(&mut self, amount: f32) {
        self.distance = (self.distance * (-amount * 0.12).exp()).clamp(0.3, 100.0);
    }

    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        let units = 2.0 * self.distance * 22.5_f32.to_radians().tan() / height.max(1.0);

        self.target += (-right * dx + up * dy) * units;
    }

    // Convert a mouse position into a ray through the scene.
    pub fn ray(&self, x: f32, y: f32, width: f32, height: f32) -> (Vec3, Vec3) {
        let inverse = self.matrix(width / height).inverse();

        let nx = 2.0 * x / width - 1.0;
        let ny = 1.0 - 2.0 * y / height;

        let near = inverse.project_point3(Vec3::new(nx, ny, 0.0));
        let far = inverse.project_point3(Vec3::new(nx, ny, 1.0));

        (near, (far - near).normalize())
    }
}
