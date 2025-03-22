use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

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
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

pub const TEST_VERTICES: &[Vertex] = &[
    Vertex { _pos: [-0.0868241, 0.49240386, 0.0] }, // A
    Vertex { _pos: [-0.49513406, 0.06958647, 0.0] }, // B
    Vertex { _pos: [-0.21918549, -0.44939706, 0.0] }, // C
    Vertex { _pos: [0.35966998, -0.3473291, 0.0] }, // D
    Vertex { _pos: [0.44147372, 0.2347359, 0.0] }, // E
];

pub const TEST_INDICES: &[u16] = &[
    0, 1, 4,
    1, 2, 4,
    2, 3, 4,
    0
];

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

#[derive(Debug)]
pub struct RenderNode {
    node: NodeData,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl RenderNode {
    pub fn new(name: String) -> Self {
        Self {
            node: NodeData::new(name),
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn get_name(&self) -> String {
        self.node.name.clone()
    }

    pub fn set_vertices(&mut self, vertices: Vec<Vertex>) {
        self.vertices = vertices;
    }

    pub fn set_indices(&mut self, indices: Vec<u16>) {
        self.indices = indices;
    }

    pub fn set_matrix(&mut self, matrix: Mat4) {
        self.node.set_matrix(matrix);
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

    pub fn add_child_to_root(&mut self, child: Node) {
        let mut root = &mut self.root;
        match root {
            Node::GroupNode(ref mut group) => {
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
    type Item = RenderNode;

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
                    let model_vertices: Vec<Vertex> = render
                        .vertices
                        .iter()
                        .map(|vertex| {
                            let pos = current_matrix.transform_point3(Vec3::new(
                                vertex._pos[0],
                                vertex._pos[1],
                                vertex._pos[2],
                            ));
                            Vertex {
                                _pos: [pos.x, pos.y, pos.z],
                            }
                        })
                        .collect();

                    let mut model = RenderNode::new(render.node.name.clone());
                    model.set_vertices(model_vertices);
                    model.set_indices(render.indices.clone());

                    return Some(model);
                }
            }
        }
        None
    }
}
