use glam::{Mat4, Vec3};
use std::time::Duration;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Camera {
    pub fn calculate_matrix(&self) -> Mat4 {
        let view = glam::Mat4::look_at_lh(self.eye, self.target, self.up);
        let projection = glam::Mat4::perspective_lh(self.fovy, self.aspect, self.znear, self.zfar);
        view * projection
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.aspect = width / height;
    }

    pub fn turn(&mut self, angle: f32) {
        let rotation = Mat4::from_rotation_y(angle);
        self.eye = rotation.transform_point3(self.eye);
        self.up = rotation.transform_vector3(self.up);
    }

    // Rotate the camera around a specified axis
    pub fn rotate(&mut self, axis: Vec3, angle: f32) {
        let rotation = Mat4::from_axis_angle(axis, angle);
        self.eye = rotation.transform_point3(self.eye);
        self.up = rotation.transform_vector3(self.up);
    }

    // Move the camera by a specified vector
    pub fn move_by(&mut self, delta: Vec3) {
        self.eye += delta;
        self.target += delta;
    }

    // Yaw the camera by a specified angle
    pub fn yaw(&mut self, angle: f32) {
        let rotation = Mat4::from_rotation_y(angle);
        self.eye = rotation.transform_point3(self.eye);
    }

    // Zoom the camera in or out by a specified factor
    pub fn zoom(&mut self, factor: f32) {
        let direction = (self.target - self.eye).normalize();
        self.eye += direction * factor;
    }
}

// Derived from: https://sotrh.github.io/learn-wgpu/intermediate/tutorial12-camera/#the-projection
#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => self.process_keyboard(*keycode, *state),
            _ => false,
        }
    }

    pub fn process_keyboard(&mut self, key: KeyCode, state: ElementState) -> bool {
        let amount = if state == ElementState::Pressed {
            1.0
        } else {
            0.0
        };
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
                true
            }
            KeyCode::Space => {
                self.amount_up = amount;
                true
            }
            KeyCode::ShiftLeft => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    pub fn process_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = -match delta {
            // I'm assuming a line is about 100 pixels
            MouseScrollDelta::LineDelta(_, scroll) => scroll * 100.0,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => *scroll as f32,
        };
    }

    pub fn update_camera(&mut self, camera: &mut Camera, dt: Duration) {
        let dt = dt.as_secs_f32();

        // Move forward/backward and left/right
        let forward = (camera.target - camera.eye).normalize();
        let right = forward.cross(camera.up).normalize();
        camera.move_by(forward * (self.amount_forward - self.amount_backward) * self.speed * dt);
        camera.move_by(right * (self.amount_right - self.amount_left) * self.speed * dt);

        // Move in/out (aka. "zoom")
        camera.zoom(self.scroll * self.speed * self.sensitivity * dt);
        self.scroll = 0.0;

        // Move up/down
        camera.move_by(Vec3::new(
            0.0,
            (self.amount_up - self.amount_down) * self.speed * dt,
            0.0,
        ));

        // Rotate
        camera.yaw(self.rotate_horizontal * self.sensitivity * dt);
        camera.rotate(camera.up, -self.rotate_vertical * self.sensitivity * dt);

        // Reset rotation values
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;
    }
}
