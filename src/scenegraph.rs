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

/*

#[derive(Debug)]
struct NodeData {
    name: String,
    children: Vec<Rc<RefCell<Node>>>,
}

impl NodeData {
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: Vec::new(),
        }
    }

    pub fn add_child(&mut self, child: Node) {
        self.children.push(Rc::new(RefCell::new(child)));
    }

    pub fn find_node(&mut self, name: &str) -> Option<&mut NodeData> {
        for child in &self.children {
            let mut child_borrow = child.borrow_mut();
            return match &mut *child_borrow {
                Node::GroupNode(group_node) => {
                    if group_node.node.name == name {
                        return Some(&mut group_node.node);
                    }
                    group_node.node.find_node(name)
                }
                Node::RenderNode(render_node) => {
                    if render_node.node.name == name {
                        return Some(&mut render_node.node);
                    }
                    render_node.node.find_node(name)
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct GroupNode {
    node: NodeData,
}

impl GroupNode {
    pub fn new(name: String) -> Self {
        Self {
            node: NodeData::new(name),
        }
    }
}

#[derive(Debug)]
pub struct RenderNode {
    node: NodeData,
}

impl RenderNode {
    pub fn new(name: String) -> Self {
        Self {
            node: NodeData::new(name),
        }
    }
}

#[derive(Debug)]
enum Node {
    GroupNode(GroupNode),
    RenderNode(RenderNode),
}

pub struct SceneGraph {
    pub root: Rc<Node>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            root: Rc::new(Node::GroupNode(GroupNode::new("root".to_string()))),
        }
    }

    pub fn add_child(&mut self, parent: &str, child: GroupNode) {
        let root = self.root.borrow_mut();
        let mut parent_node = self.find_node(root, parent).unwrap();
        parent_node.children.push(Rc::new(RefCell::new(Node::GroupNode(child))));
    }

    pub fn find_node(&self, node: &mut Node, name: &str) -> Option<&mut NodeData> {
        let mut node = match node {
            Node::GroupNode(group_node) => &group_node.node,
            Node::RenderNode(render_node) => &render_node.node,
        };
        node.find_node(name)
    }

    pub fn generate_buffers(&self) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        self.generate_buffers_recursive(&self.root, &mut vertices, &mut indices);
    }

    fn generate_buffers_recursive(
        &self,
        node: &Node,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
    ) {
        match node {
            Node::GroupNode(group_node) => {
                for child in &group_node.node.children {
                    self.generate_buffers_recursive(&child.borrow(), vertices, indices);
                }
            }
            Node::RenderNode(render_node) => {
                // Generate vertices and indices
            }
        }
    }
}*/