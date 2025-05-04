use crate::light::{Light, LightUniform, ShadowMap};
use crate::model;
use crate::model::Vertex;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::{DeviceExt};
use wgpu::{BindGroup, BindGroupLayout, Buffer, Queue, RenderPass};

#[derive(Debug)]
pub struct NodeData {
    name: String,
    matrix: Mat4,
}

impl NodeData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            matrix: Mat4::IDENTITY,
        }
    }

    pub fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = matrix;
    }
}

#[derive(Debug)]
pub struct GroupNode {
    node: NodeData,
    pub children: Vec<Node>,
}

impl GroupNode {
    pub fn new(name: String) -> Self {
        Self {
            node: NodeData::new(name),
            children: Vec::new(),
        }
    }

    pub fn set_matrix(&mut self, matrix: Mat4) {
        self.node.set_matrix(matrix);
    }

    pub fn add_child(&mut self, child: Node) {
        self.children.push(child);
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelUniform {
    view_proj: [[f32; 4]; 4],
}

#[derive(Debug)]
pub struct RenderNode {
    node: NodeData,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material_bind_group: Option<BindGroup>,
    vertices: Vec<Vertex>,
}

#[derive(Debug)]
pub struct LightNode {
    pub node: NodeData,
    pub light: Light,
}

impl RenderNode {
    fn new(
        name: String,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        material_bind_group: Option<wgpu::BindGroup>,
    ) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Vertex Buffer", name)),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Index Buffer", name)),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            node: NodeData::new(name),
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
            material_bind_group,
            vertices: vertices.to_vec(),
        }
    }

    fn new_with_matrix(
        name: String,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        material_bind_group: Option<wgpu::BindGroup>,
        matrix: Mat4,
    ) -> Self {
        let mut render_node = Self::new(name, device, vertices, indices, material_bind_group);
        render_node.set_matrix(matrix, device);
        render_node
    }

    pub fn set_matrix(&mut self, matrix: Mat4, device: &wgpu::Device) {
        self.node.set_matrix(matrix);
        let transformed_vertices: Vec<Vertex> = self
            .vertices
            .iter()
            .map(|vertex| {
                let pos =
                    matrix.transform_point3(Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]));
                Vertex {
                    pos: [pos.x, pos.y, pos.z],
                    ..*vertex
                }
            })
            .collect();

        self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Vertex Buffer", self.node.name)),
            contents: bytemuck::cast_slice(&transformed_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
    }
}

#[derive(Debug)]
pub enum Node {
    GroupNode(GroupNode),
    RenderNode(RenderNode),
    LightNode(LightNode),
}

pub struct SceneGraph {
    pub root: Node,
    pub light_bind_group: Option<BindGroup>,
    pub light_bind_group_layout: Option<BindGroupLayout>,
    pub lights_dirty: bool,
    pub supports_storage_resources: bool,
    pub shadow_map: ShadowMap,
    on_frame_update_callback: Option<Box<dyn Fn(&SceneGraph)>>,
}

impl SceneGraph {
    pub fn new(supports_storage_resources: bool, shadow_map: ShadowMap) -> Self {
        Self {
            root: Node::GroupNode(GroupNode::new("root".to_string())),
            light_bind_group: None,
            light_bind_group_layout: None,
            lights_dirty: false,
            supports_storage_resources,
            shadow_map,
            on_frame_update_callback: None,
        }
    }

    pub fn add_render_node(
        &mut self,
        parent: Option<&str>,
        name: String,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        matrix: Mat4,
    ) {
        let render_node =
            RenderNode::new_with_matrix(name, device, vertices, indices, None, matrix);
        self.add_child(parent, Node::RenderNode(render_node));
    }

    pub fn add_model_node(
        &mut self,
        parent: Option<&str>,
        name: String,
        device: &wgpu::Device,
        model: &model::Model,
        bind_group_layout: &BindGroupLayout,
        matrix: Mat4,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            let bind_group = material.create_bind_group(device, &bind_group_layout);

            let render_node = RenderNode::new_with_matrix(
                format!("{}-{}", name, mesh.name),
                device,
                &mesh.vertices,
                &mesh.indices,
                bind_group,
                matrix,
            );
            self.add_child(parent, Node::RenderNode(render_node));
        }
    }

    pub fn add_light_node(
        &mut self,
        parent: Option<&str>,
        name: String,
        device: &wgpu::Device,
        light: Light,
    ) {
        let light_node = LightNode {
            node: NodeData::new(name),
            light,
        };
        self.add_child(parent, Node::LightNode(light_node));
        self.update_light_bind_group(device);
    }

    fn add_child(&mut self, parent: Option<&str>, child: Node) {
        let parent_node = self.find_child_mut(parent).unwrap();
        if let Node::GroupNode(ref mut group) = parent_node {
            group.children.push(child);
        }
    }

    pub fn find_child(&self, name: &str) -> Option<&Node> {
        self.find_child_deep(name)
    }

    pub fn find_child_mut(&mut self, name: Option<&str>) -> Option<&mut Node> {
        if let Some(name) = name {
            self.find_child_mut_deep(name)
        } else {
            Some(&mut self.root)
        }
    }

    /*
     * Iterative function to find a child node by name.
     * An iterative function is used since Rust prefers it over recursion.
     */
    fn find_child_deep(&self, name: &str) -> Option<&Node> {
        let mut stack = vec![&self.root];
        while let Some(node) = stack.pop() {
            match node {
                Node::GroupNode(group) => {
                    if group.node.name == name {
                        return Some(node);
                    }
                    for child in &group.children {
                        stack.push(child);
                    }
                }
                Node::RenderNode(render) => {
                    if render.node.name == name {
                        return Some(node);
                    }
                }
                Node::LightNode(light) => {
                    if light.node.name == name {
                        return Some(node);
                    }
                }
            }
        }
        None
    }

    fn find_child_mut_deep(&mut self, name: &str) -> Option<&mut Node> {
        let mut stack = vec![&mut self.root];
        while let Some(node) = stack.pop() {
            match node {
                Node::GroupNode(group) => {
                    for child in &mut group.children {
                        match child {
                            Node::GroupNode(group) => {
                                if group.node.name == name {
                                    return Some(child);
                                }
                                stack.push(child);
                            }
                            Node::RenderNode(render) => {
                                if render.node.name == name {
                                    return Some(child);
                                }
                            }
                            Node::LightNode(render) => {
                                if render.node.name == name {
                                    return Some(child);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn get_light_nodes(&self) -> Vec<(&LightNode, Mat4)> {
        SceneGraphLightNodeIterator::new(self).collect::<Vec<(_, _)>>()
    }

    fn get_light_uniforms(&self) -> Vec<LightUniform> {
        let mut uniforms = vec![];
        for light in self.get_light_nodes() {
            let uniform = LightUniform::from_light(&light.0.light, light.1);
            uniforms.push(uniform);
        }
        uniforms
    }

    pub fn update_light_bind_group_layout(&mut self, device: &wgpu::Device) {
        let light_count = self.get_light_nodes().len() as u32;
        if light_count == 0 {
            self.light_bind_group_layout = None;
        }
        self.light_bind_group_layout = Some(LightUniform::get_bind_group_layout(
            device,
            light_count,
            self.supports_storage_resources,
        ));
    }

    pub fn update_light_bind_group(&mut self, device: &wgpu::Device) {
        self.lights_dirty = true;
        let light_uniforms = self.get_light_uniforms();
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Buffer"),
            contents: bytemuck::cast_slice(&light_uniforms),
            usage: if self.supports_storage_resources {
                wgpu::BufferUsages::STORAGE
            } else {
                wgpu::BufferUsages::UNIFORM
            } | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        self.update_light_bind_group_layout(device);
        if let Some(light_bind_group_layout) = &self.light_bind_group_layout {
            self.light_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &light_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: light_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.shadow_map.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.shadow_map.sampler),
                    },
                ],
                label: Some("Light Bind Group"),
            }));
        }
    }
    
    pub fn set_callback (&mut self, callback: Box<dyn Fn(&SceneGraph)>) {
        self.on_frame_update_callback = Some(callback);
    }

    pub fn on_frame_update(&mut self) {
        self.lights_dirty = false;
        if let Some(callback) = &self.on_frame_update_callback {
            callback(self);
        }
    }
}

pub struct SceneGraphRenderNodeIterator<'a> {
    stack: Vec<(&'a Node, Mat4)>,
}

impl<'a> SceneGraphRenderNodeIterator<'a> {
    pub fn new(scene_graph: &'a SceneGraph) -> Self {
        Self {
            stack: vec![(&scene_graph.root, Mat4::IDENTITY)],
        }
    }
}

impl<'a> Iterator for SceneGraphRenderNodeIterator<'a> {
    type Item = (&'a RenderNode, Mat4);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, parent_matrix)) = self.stack.pop() {
            match node {
                Node::GroupNode(group) => {
                    let current_matrix = parent_matrix * group.node.matrix;
                    for child in &group.children {
                        self.stack.push((child, current_matrix));
                    }
                }
                Node::RenderNode(render) => {
                    let current_matrix = parent_matrix * render.node.matrix;
                    return Some((render, current_matrix));
                }
                _ => {}
            }
        }
        None
    }
}

pub struct SceneGraphLightNodeIterator<'a> {
    stack: Vec<(&'a Node, Mat4)>,
}

impl<'a> SceneGraphLightNodeIterator<'a> {
    pub fn new(scene_graph: &'a SceneGraph) -> Self {
        Self {
            stack: vec![(&scene_graph.root, Mat4::IDENTITY)],
        }
    }
}

impl<'a> Iterator for SceneGraphLightNodeIterator<'a> {
    type Item = (&'a LightNode, Mat4);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((node, parent_matrix)) = self.stack.pop() {
            match node {
                Node::GroupNode(group) => {
                    let current_matrix = parent_matrix * group.node.matrix;
                    for child in &group.children {
                        self.stack.push((child, current_matrix));
                    }
                }
                Node::LightNode(light) => {
                    let current_matrix = parent_matrix * light.node.matrix;
                    return Some((light, current_matrix));
                }
                _ => {}
            }
        }
        None
    }
}

pub trait DrawScenegraph<'a> {
    fn draw_scenegraph(
        &mut self,
        scenegraph: &'a SceneGraph,
        queue: &Queue,
        material_bind_group_index: u32,
        model_mat_buffer: &Buffer,
        camera_position: &Vec3,
    );

    fn draw_scenegraph_vertices(
        &mut self,
        scenegraph: &'a SceneGraph,
        queue: &Queue,
        model_mat_buffer: &Buffer,
    );
}

impl<'a, 'b> DrawScenegraph<'b> for RenderPass<'a>
where
    'b: 'a,
{
    fn draw_scenegraph(
        &mut self,
        scenegraph: &'b SceneGraph,
        queue: &Queue,
        material_bind_group_index: u32,
        model_mat_buffer: &Buffer,
        camera_position: &Vec3,
    ) {
        let iterator = SceneGraphRenderNodeIterator::new(scenegraph);
        let render_nodes: Vec<(&RenderNode, Mat4)> = iterator.collect();

        for render_node in render_nodes {
            self.set_vertex_buffer(0, render_node.0.vertex_buffer.slice(..));
            self.set_index_buffer(
                render_node.0.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            queue.write_buffer(
                model_mat_buffer,
                0,
                bytemuck::cast_slice(&[ModelUniform {
                    view_proj: render_node.1.to_cols_array_2d(),
                }]),
            );
            if let Some(material_bind_group) = &render_node.0.material_bind_group {
                self.set_bind_group(material_bind_group_index, material_bind_group, &[]);
            } else {
                self.set_bind_group(material_bind_group_index, None, &[]);
                println!(
                    "Material bind group not found for {}",
                    render_node.0.node.name
                );
            }
            self.draw_indexed(0..render_node.0.num_elements, 0, 0..1);
        }
    }

    fn draw_scenegraph_vertices(
        &mut self,
        scenegraph: &'b SceneGraph,
        queue: &Queue,
        model_mat_buffer: &Buffer,
    ) {
        let iterator = SceneGraphRenderNodeIterator::new(scenegraph);
        let render_nodes: Vec<(&RenderNode, Mat4)> = iterator.collect();

        for render_node in render_nodes {
            queue.write_buffer(
                model_mat_buffer,
                0,
                bytemuck::cast_slice(&[ModelUniform {
                    view_proj: render_node.1.to_cols_array_2d(),
                }]),
            );

            self.set_vertex_buffer(0, render_node.0.vertex_buffer.slice(..));
            self.set_index_buffer(
                render_node.0.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            self.draw_indexed(0..render_node.0.num_elements, 0, 0..1);
        }
    }
}
