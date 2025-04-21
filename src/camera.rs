use glam::{Mat3, Mat4, Vec3};
use std::clone::Clone;
use winit::event::{ElementState, KeyEvent, WindowEvent};
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 4],
}

impl CameraUniform {
    pub fn from_camera(camera: &Camera) -> Self {
        Self {
            view_proj: camera.calculate_matrix().to_cols_array_2d(),
            position: [camera.eye.x, camera.eye.y, camera.eye.z, 1.0],
        }
    }

    pub fn update(&mut self, camera: &Camera) {
        self.view_proj = camera.calculate_matrix().to_cols_array_2d();
        self.position = [camera.eye.x, camera.eye.y, camera.eye.z, 1.0];
    }

    pub fn get_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        })
    }
}

impl Camera {
    pub fn calculate_matrix(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, self.up);
        let projection = Mat4::perspective_rh(self.fovy.to_radians(), self.aspect, self.znear, self.zfar);
        projection * view
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

    pub fn get_focal_point(&self) -> Vec3 {
        self.eye + (self.target - self.eye).normalize() * 10.0
    }
}

// Derived from: https://sotrh.github.io/learn-wgpu/beginner/tutorial6-uniforms/#a-controller-for-our-camera
pub struct CameraController {
    speed: f32,
    sensitivity: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_mouse_pressed: bool,
    is_up_pressed: bool,
    is_down_pressed: bool,
    delta_x: f64,
    delta_y: f64,
    last_mouse_position: Option<(f64, f64)>,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_mouse_pressed: false,
            is_up_pressed: false,
            is_down_pressed: false,
            delta_x: 0.0,
            delta_y: 0.0,
            last_mouse_position: None,
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
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyA | KeyCode::ArrowLeft => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    KeyCode::KeyD | KeyCode::ArrowRight => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    KeyCode::Space => {
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == winit::event::MouseButton::Left {
                    self.is_mouse_pressed = *state == ElementState::Pressed;
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.is_mouse_pressed {
                    if let Some((last_x, last_y)) = self.last_mouse_position {
                        self.delta_x = position.x - last_x;
                        self.delta_y = position.y - last_y;
                    }
                } else {
                    self.delta_x = 0.0;
                    self.delta_y = 0.0;
                }
                self.last_mouse_position = Some((position.x, position.y));
                true
            }
            _ => false,
        }
    }

    pub fn update_camera(&self, camera: &mut Camera) {
        let forward = (camera.target - camera.eye).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = camera.up.normalize();

        if self.is_forward_pressed {
            camera.move_by(forward * self.speed);
        }
        if self.is_backward_pressed {
            camera.move_by(-forward * self.speed);
        }
        if self.is_right_pressed {
            camera.move_by(right * self.speed);
        }
        if self.is_left_pressed {
            camera.move_by(-right * self.speed);
        }
        if self.is_up_pressed {
            camera.move_by(up * self.speed);
        }
        if self.is_down_pressed {
            camera.move_by(-up * self.speed);
        }

        // Verhindere, dass die Kamera unter den Boden geht
        if camera.eye.y <= 0.0 {
            camera.eye.y = 0.1;
        }

        if self.is_mouse_pressed {
            let delta_x = self.delta_x as f32 * self.sensitivity;
            let delta_y = self.delta_y as f32 * self.sensitivity;

            let rotation_x = Mat3::from_rotation_y(delta_x.to_radians());
            let rotation_y = Mat3::from_axis_angle(right, -delta_y.to_radians());

            let new_forward = rotation_y * rotation_x * forward;
            camera.target = camera.eye + new_forward;
            camera.up = rotation_y * rotation_x * camera.up;
        }
    }
}

