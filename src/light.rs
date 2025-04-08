use std::num::NonZeroU32;
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightUniform {
    pos: [f32; 4],
    color: [f32; 4],
}

impl LightUniform {
    pub fn from_light(light: &Light) -> Self {
        Self {
            pos: [light.pos.x, light.pos.y, light.pos.z, 1.0],
            color: [light.color.r as f32, light.color.g as f32, light.color.b as f32, light.color.a as f32],
        }
    }

    pub fn get_bind_group_layout(device: &wgpu::Device, light_count: u32) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: NonZeroU32::new(light_count),
            }],
            label: Some("light_bind_group_layout"),
        })
    }
}

#[derive(Debug)]
pub struct Light {
    pos: glam::Vec3,
    color: wgpu::Color,
}

impl Light {
    pub fn new(pos: glam::Vec3, color: wgpu::Color) -> Self {
        Self {
            pos,
            color,
        }
    }
}