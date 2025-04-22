/*
   Taken (mostly) from https://sotrh.github.io/learn-wgpu/beginner/tutorial9-models/#loading-models-with-tobj
*/
use crate::resources::{load_string, load_texture};
use crate::texture;
use crate::texture::get_default_texture;
use bytemuck::{Pod, Zeroable};
use std::io::{BufReader, Cursor};
use std::ops::Range;
use wasm_bindgen::throw_str;
use wgpu::util::DeviceExt;
use wgpu::Device;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub const CUBE_VERTICES: &[Vertex] = &[
    Vertex { pos: [-1.0, -1.0,  1.0], tex_coords: [0.0, 0.0], normal: [0.0, 0.0,  1.0] },
    Vertex { pos: [ 1.0, -1.0,  1.0], tex_coords: [1.0, 0.0], normal: [0.0, 0.0,  1.0] },
    Vertex { pos: [ 1.0,  1.0,  1.0], tex_coords: [1.0, 1.0], normal: [0.0, 0.0,  1.0] },
    Vertex { pos: [-1.0,  1.0,  1.0], tex_coords: [0.0, 1.0], normal: [0.0, 0.0,  1.0] },

    Vertex { pos: [-1.0, -1.0, -1.0], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, -1.0] },
    Vertex { pos: [ 1.0, -1.0, -1.0], tex_coords: [1.0, 0.0], normal: [0.0, 0.0, -1.0] },
    Vertex { pos: [ 1.0,  1.0, -1.0], tex_coords: [1.0, 1.0], normal: [0.0, 0.0, -1.0] },
    Vertex { pos: [-1.0,  1.0, -1.0], tex_coords: [0.0, 1.0], normal: [0.0, 0.0, -1.0] },

    Vertex { pos: [-1.0, -1.0, -1.0], tex_coords: [0.0, 0.0], normal: [-1.0, 0.0, 0.0] },
    Vertex { pos: [-1.0, -1.0,  1.0], tex_coords: [1.0, 0.0], normal: [-1.0, 0.0, 0.0] },
    Vertex { pos: [-1.0,  1.0,  1.0], tex_coords: [1.0, 1.0], normal: [-1.0, 0.0, 0.0] },
    Vertex { pos: [-1.0,  1.0, -1.0], tex_coords: [0.0, 1.0], normal: [-1.0, 0.0, 0.0] },

    Vertex { pos: [ 1.0, -1.0, -1.0], tex_coords: [0.0, 0.0], normal: [1.0, 0.0, 0.0] },
    Vertex { pos: [ 1.0, -1.0,  1.0], tex_coords: [1.0, 0.0], normal: [1.0, 0.0, 0.0] },
    Vertex { pos: [ 1.0,  1.0,  1.0], tex_coords: [1.0, 1.0], normal: [1.0, 0.0, 0.0] },
    Vertex { pos: [ 1.0,  1.0, -1.0], tex_coords: [0.0, 1.0], normal: [1.0, 0.0, 0.0] },

    Vertex { pos: [-1.0,  1.0, -1.0], tex_coords: [0.0, 0.0], normal: [0.0, 1.0, 0.0] },
    Vertex { pos: [-1.0,  1.0,  1.0], tex_coords: [1.0, 0.0], normal: [0.0, 1.0, 0.0] },
    Vertex { pos: [ 1.0,  1.0,  1.0], tex_coords: [1.0, 1.0], normal: [0.0, 1.0, 0.0] },
    Vertex { pos: [ 1.0,  1.0, -1.0], tex_coords: [0.0, 1.0], normal: [0.0, 1.0, 0.0] },

    Vertex { pos: [-1.0, -1.0, -1.0], tex_coords: [0.0, 0.0], normal: [0.0, -1.0, 0.0] },
    Vertex { pos: [-1.0, -1.0,  1.0], tex_coords: [1.0, 0.0], normal: [0.0, -1.0, 0.0] },
    Vertex { pos: [ 1.0, -1.0,  1.0], tex_coords: [1.0, 1.0], normal: [0.0, -1.0, 0.0] },
    Vertex { pos: [ 1.0, -1.0, -1.0], tex_coords: [0.0, 1.0], normal: [0.0, -1.0, 0.0] },
];

pub const CUBE_INDICES: &[u32] = &[
    0, 1, 2, 2, 3, 0,
    4, 5, 6, 6, 7, 4,
    8, 9, 10, 10, 11, 8,
    12, 13, 14, 14, 15, 12,
    16, 17, 18, 18, 19, 16,
    20, 21, 22, 22, 23, 20,
];

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MaterialUniform {
    pub ambient: [f32; 4],
    pub diffuse: [f32; 4],
    pub specular: [f32; 4],
    pub shininess: f32,
    pub dissolve: f32,
    _padding: [f32; 2],
}

impl MaterialUniform {
    pub fn from_tobj_material(material: &tobj::Material) -> Self {
        // Convert the ambient color to a 4-component vector
        // by adding a padding bit
        let ambient = material.ambient.unwrap_or([0.0; 3]);
        let diffuse = material.diffuse.unwrap_or([0.0; 3]);
        let specular = material.specular.unwrap_or([0.0; 3]);

        Self {
            ambient: [ambient[0], ambient[1], ambient[2], 0.0].into(),
            diffuse: [diffuse[0], diffuse[1], diffuse[2], 0.0].into(),
            specular: [specular[0], specular[1], specular[2], 0.0].into(),
            shininess: material.shininess.unwrap_or(1.0),
            dissolve: material.dissolve.unwrap_or(1.0),
            _padding: [0.0; 2],
        }
    }
}

#[derive(Debug)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

#[derive(Debug, Default)]
pub struct Material {
    pub name: String,
    pub diffuse_texture: Option<texture::Texture>,
    pub material: tobj::Material,
}

impl Material {
    pub fn new(
        name: &str,
        diffuse_color: Option<[f32; 3]>,
        device: &Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let default_texture =
            texture::Texture::from_image(device, queue, &get_default_texture(), Some("ground"))
                .unwrap_or_else(|e| throw_str(&format!("{e:#?}")));

        let material = tobj::Material {
                name: name.to_string(),
                diffuse: diffuse_color,
                dissolve: Some(1.0),
                ..Default::default()
        };
        Self {
            name: name.to_string(),
            diffuse_texture: Some(default_texture),
            material,
        }
    }
    pub fn create_bind_group(
        &self,
        device: &Device,
        layout: &wgpu::BindGroupLayout,
    ) -> Option<wgpu::BindGroup> {
        let material_uniform = MaterialUniform::from_tobj_material(&self.material);
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Buffer"),
            contents: bytemuck::cast_slice(&[material_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        if let Some(diffuse_texture) = &self.diffuse_texture {
            return Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &material_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                ],
                label: Some(&self.name),
            }))
        }
        None
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub num_elements: u32,
    pub material: usize,
}

pub async fn load_model(
    file_path: &str,
    file_name: &str,
    device: &Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Model> {
    let full_path = std::path::Path::new(&file_path).join(file_name);
    let obj_text = load_string(full_path.to_str().unwrap()).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            // Replace the file path with the path to the material file
            let material_path = std::path::Path::new(&file_path).join(&p);
            let mat_text = load_string(material_path.to_str().unwrap()).await.unwrap();
            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    )
    .await?;

    let mut materials = Vec::new();
    for m in obj_materials? {
        if m.diffuse_texture.is_none() {
            materials.push(Material {
                name: m.name.clone(),
                diffuse_texture: Some(texture::Texture::from_image(device, queue, &get_default_texture(), Some(m.name.as_str()))?),
                material: m.clone(),
            });
            continue;
        }
        let material = m.clone();
        let texture_path = std::path::Path::new(&file_path).join(&m.diffuse_texture.unwrap());
        let diffuse_texture = Some(load_texture(texture_path.to_str(), device, queue).await?);

        materials.push(Material {
            name: m.name,
            diffuse_texture,
            material,
        });
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| {
                    if m.mesh.normals.is_empty() {
                        Vertex {
                            pos: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                            tex_coords: [
                                m.mesh.texcoords[i * 2],
                                1.0 - m.mesh.texcoords[i * 2 + 1],
                            ],
                            normal: [0.0, 0.0, 0.0],
                        }
                    } else {
                        Vertex {
                            pos: [
                                m.mesh.positions[i * 3],
                                m.mesh.positions[i * 3 + 1],
                                m.mesh.positions[i * 3 + 2],
                            ],
                            tex_coords: [
                                m.mesh.texcoords[i * 2],
                                1.0 - m.mesh.texcoords[i * 2 + 1],
                            ],
                            normal: [
                                m.mesh.normals[i * 3],
                                m.mesh.normals[i * 3 + 1],
                                m.mesh.normals[i * 3 + 2],
                            ],
                        }
                    }
                })
                .collect::<Vec<_>>();

            let len = m.mesh.indices.len() as u32;

            Mesh {
                name: full_path.to_str().unwrap().to_string(),
                vertices,
                indices: m.mesh.indices,
                num_elements: len,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(Model { meshes, materials })
}

pub trait DrawModel<'a> {
    fn draw_mesh(&mut self, mesh: &'a Mesh);
    fn draw_mesh_instanced(&mut self, mesh: &'a Mesh, instances: Range<u32>);
}
