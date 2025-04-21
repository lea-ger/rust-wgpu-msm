use crate::camera::{Camera, CameraUniform};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::num::NonZeroU32;
use wgpu::{Texture, TextureView};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightUniform {
    pos: [f32; 4],
    color: [f32; 4],
    model_mat: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
}

impl LightUniform {
    pub fn from_light(light: &Light, model: Mat4) -> Self {
        Self {
            pos: [light.pos.x, light.pos.y, light.pos.z, 1.0],
            color: [
                light.color.r as f32,
                light.color.g as f32,
                light.color.b as f32,
                light.color.a as f32,
            ],
            model_mat: model.to_cols_array_2d(),
            view_proj: light.get_view_proj(model).to_cols_array_2d(),
        }
    }

    pub fn get_bind_group_layout(
        device: &wgpu::Device,
        light_count: u32,
        supports_storage_resources: bool,
    ) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: if supports_storage_resources {
                            wgpu::BufferBindingType::Storage { read_only: true }
                        } else {
                            wgpu::BufferBindingType::Uniform
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: NonZeroU32::new(light_count),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
            label: Some("light_bind_group_layout"),
        })
    }
}

#[derive(Debug)]
pub struct Light {
    pub pos: Vec3,
    color: wgpu::Color,
    pub target_view: TextureView,
}


impl Light {
    pub fn new(pos: Vec3, color: wgpu::Color, shadow_texture: &Texture, light_number: u32) -> Self {
        Self {
            pos,
            color,
            target_view: shadow_texture.create_view(
            &wgpu::TextureViewDescriptor {
                label: Some("shadow"),
                format: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                usage: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: light_number,
                array_layer_count: Some(1),
            }),
        }
    }

    pub fn get_view_proj(&self, model: Mat4) -> Mat4 {
        let pos4 = glam::Vec4::new(self.pos.x, self.pos.y, self.pos.z, 1.0);
        let position = model * pos4;
        let view = Mat4::look_at_rh(position.truncate(), Vec3::ZERO, Vec3::Y);
        let projection = Mat4::perspective_rh(0.0f32.to_radians(), 1., 0.0, 100.0);
        view * projection
    }

    pub fn to_camera_uniform(&self, model: Mat4) -> CameraUniform {
        let pos4 = glam::Vec4::new(self.pos.x, self.pos.y, self.pos.z, 1.0);
        let position = model * pos4;
        CameraUniform {
            view_proj: self.get_view_proj(model).to_cols_array_2d(),
            position: [
                position.x,
                position.y,
                position.z,
                1.0,
            ],
        }
    }
}

pub struct ShadowMap {
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: wgpu::Sampler,
}

impl ShadowMap {
    pub const MAX_LIGHTS: u32 = 3;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn create_shadow_map(device: &wgpu::Device, size: u32) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: Self::MAX_LIGHTS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow Map View"),
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
        }
    }
}
