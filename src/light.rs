use crate::camera::CameraUniform;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::num::NonZeroU32;
use wgpu::{Texture, TextureUsages, TextureView};

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
            view_proj: light.calculate_matrix(model).to_cols_array_2d(),
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
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: false,
                        },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
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
            target_view: shadow_texture.create_view(&wgpu::TextureViewDescriptor {
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

    pub fn calculate_matrix(&self, model: Mat4) -> Mat4 {
        let pos4 = glam::Vec4::new(self.pos.x, self.pos.y, self.pos.z, 1.0);
        let position = model * pos4;
        let center = Vec3::new(0.0, 0.0, -15.0);
        let view = Mat4::look_at_rh(position.truncate(), center, Vec3::Y);
        let projection = Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 5.0, 50.0);
        projection * view
    }

    pub fn to_camera_uniform(&self, model: Mat4) -> CameraUniform {
        CameraUniform {
            view_proj: self.calculate_matrix(model).to_cols_array_2d(),
            position: [self.pos.x, self.pos.y, self.pos.z, 1.0],
        }
    }
}

#[derive(Clone)]
pub struct ShadowMap {
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: wgpu::Sampler,
}

impl ShadowMap {
    pub const MAX_LIGHTS: u32 = 3;
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    pub const SHADOW_MAP_SIZE: u32 = 2048;

    pub fn create_shadow_map(device: &wgpu::Device, usages: Option<TextureUsages>) -> Self {
        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Map"),
            size: wgpu::Extent3d {
                width: Self::SHADOW_MAP_SIZE,
                height: Self::SHADOW_MAP_SIZE,
                depth_or_array_layers: Self::MAX_LIGHTS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: if let Some(usages) = usages {
                usages
            } else {
                TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::TEXTURE_BINDING
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_SRC
            },
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: None,
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
