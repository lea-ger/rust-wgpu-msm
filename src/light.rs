use bytemuck::{Pod, Zeroable};
use std::num::NonZeroU32;
use glam::Mat4;
use crate::camera::{Camera, CameraUniform};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightUniform {
    pos: [f32; 4],
    color: [f32; 4],
    model_mat: [[f32; 4]; 4],
}

impl LightUniform {
    pub fn from_light(light: &Light, model: Mat4) -> Self {
        Self {
            pos: [light.pos.x, light.pos.y, light.pos.z, 1.0],
            color: [light.color.r as f32, light.color.g as f32, light.color.b as f32, light.color.a as f32],
            model_mat: model.to_cols_array_2d(),
        }
    }

    pub fn get_bind_group_layout(device: &wgpu::Device, light_count: u32, supports_storage_resources: bool) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
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
            }],
            label: Some("light_bind_group_layout"),
        })
    }
}

#[derive(Debug)]
pub struct Light {
    pub pos: glam::Vec3,
    color: wgpu::Color,
    pub target_view: Option<wgpu::TextureView>,
}

impl Light {
    pub fn new(pos: glam::Vec3, color: wgpu::Color) -> Self {
        Self {
            pos,
            color,
            target_view: None,
        }
    }

    pub fn to_camera_uniform(&self, camera: &Camera) -> CameraUniform {
        CameraUniform {
            view_proj: Mat4::look_at_rh(self.pos, camera.target, glam::Vec3::Y).to_cols_array_2d(),
            position: [self.pos.x, self.pos.y, self.pos.z, 1.0],
        }
    }
}

struct ShadowMap {
    texture: wgpu::Texture,
    sampler: wgpu::Sampler,
}

impl ShadowMap {
    pub fn create_shadow_map(device: &wgpu::Device, size: u32) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
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
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });

        Self {
            texture,
            sampler,
        }
    }

    pub fn create_target_view_from_light(&self, device: &wgpu::Device) -> wgpu::TextureView {
        self.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Shadow Map View"),
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        })
    }
}