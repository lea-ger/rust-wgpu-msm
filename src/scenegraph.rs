use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub _pos: [f32; 3],
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
            ]
        }
    }
}

pub const TEST_VERTICES: &[Vertex] = &[
    Vertex { _pos: [0.0, 0.5, 0.0] },
    Vertex { _pos: [-0.5, -0.5, 0.0] },
    Vertex { _pos: [0.5, -0.5, 0.0] },
];


#[derive(Debug)]
struct NodeData {
    name: String,
    matrix: glam::Mat4,
}

impl NodeData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            matrix: glam::Mat4::IDENTITY,
        }
    }

    pub fn set_matrix(&mut self, matrix: glam::Mat4) {
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
}

#[derive(Debug)]
pub struct RenderNode {
    node: NodeData,
    vertices: Vec<Vertex>,
}

impl RenderNode {
    pub fn new(name: String) -> Self {
        Self {
            node: NodeData::new(name),
            vertices: Vec::new(),
        }
    }

    pub fn set_vertices(&mut self, vertices: Vec<Vertex>) {
        self.vertices = vertices;
    }
}

#[derive(Debug)]
enum Node {
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

    pub fn add_child_to_root(self, child: Node) {
        let root = self.root;
        match root {
            Node::GroupNode(mut group) => {
                group.children.push(child);
            }
            _ => {}
        }
    }

    pub fn add_child(&mut self, parent: &str, child: Node) {
        let mut parent_node = self.find_child_mut(parent).unwrap();
        if let Node::GroupNode(ref mut group) = parent_node {
            group.children.push(child);
        }
    }

    pub fn find_child(&self, name: &str) -> Option<&Node> {
        self.find_child_deep(name)
    }

    pub fn find_child_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.find_child_mut_deep(name)
    }

    /*
    * Iterative function to find a child node by name.
    * An iterative function is used since Rust prefers it over recursion.
     */
    fn find_child_deep<'a>(&'a self, name: &str) -> Option<&'a Node> {
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