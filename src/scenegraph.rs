use crate::model;
use crate::model::Vertex;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use wgpu::RenderPass;

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

trait Renderable {
    fn render(&self);
}

#[derive(Debug)]
pub struct RenderNode {
    node: NodeData,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    vertices: Vec<Vertex>,
}

impl RenderNode {
    fn new(name: String, device: &wgpu::Device, vertices: &[Vertex], indices: &[u32]) -> Self {
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
            vertices: vertices.to_vec(),
        }
    }

    fn new_with_matrix(
        name: String,
        device: &wgpu::Device,
        vertices: &[Vertex],
        indices: &[u32],
        matrix: Mat4,
    ) -> Self {
        let mut render_node = Self::new(name, device, vertices, indices);
        render_node.set_matrix(matrix, device);
        render_node
    }

    fn set_matrix(&mut self, matrix: Mat4, device: &wgpu::Device) {
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
}

pub struct SceneGraph {
    pub root: Node,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            root: Node::GroupNode(GroupNode::new("root".to_string())),
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
        let render_node = RenderNode::new_with_matrix(name, device, vertices, indices, matrix);
        self.add_child(parent, Node::RenderNode(render_node));
    }

    pub fn add_model_node(
        &mut self,
        parent: Option<&str>,
        name: String,
        device: &wgpu::Device,
        model: &model::Model,
        matrix: Mat4,
    ) {
        for mesh in &model.meshes {
            let render_node = RenderNode::new_with_matrix(
                format!("{}-{}", name, mesh.name),
                device,
                &mesh.vertices,
                &mesh.indices,
                matrix,
            );
            self.add_child(parent, Node::RenderNode(render_node));
        }
    }

    pub fn add_child(&mut self, parent: Option<&str>, child: Node) {
        let mut parent_node = self.find_child_mut(parent).unwrap();
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
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
}

pub struct SceneGraphIterator<'a> {
    stack: Vec<(&'a Node, Mat4)>,
}

impl<'a> SceneGraphIterator<'a> {
    pub fn new(scene_graph: &'a SceneGraph) -> Self {
        Self {
            stack: vec![(&scene_graph.root, Mat4::IDENTITY)],
        }
    }
}

impl<'a> Iterator for SceneGraphIterator<'a> {
    type Item = &'a RenderNode;

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
                    return Some(render);
                }
            }
        }
        None
    }
}

pub trait DrawScenegraph<'a> {
    fn draw_scenegraph(&mut self, scenegraph: &'a SceneGraph);
}

impl<'a, 'b> DrawScenegraph<'b> for RenderPass<'a>
where
    'b: 'a,
{
    fn draw_scenegraph(&mut self, scenegraph: &'b SceneGraph) {
        let iterator = SceneGraphIterator::new(scenegraph);
        for render_node in iterator {
            self.set_vertex_buffer(0, render_node.vertex_buffer.slice(..));
            self.set_index_buffer(render_node.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_indexed(0..render_node.num_elements, 0, 0..1);
        }
    }
}