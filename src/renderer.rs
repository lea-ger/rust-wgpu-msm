use crate::scenegraph::{
    GroupNode, Node, RenderNode, SceneGraph, SceneGraphIterator, Vertex, TEST_VERTICES,
};
use glam::{Mat4, Vec3};
use std::borrow::Cow;
use std::future::Future;
use std::sync::{Arc, Mutex};
use wasm_bindgen::{throw_str, UnwrapThrowExt};
use wgpu::util::DeviceExt;
use wgpu::{Adapter, Device, Instance, Queue, RenderPipeline, Surface, SurfaceConfiguration};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;
use crate::camera::{Camera, CameraController, CameraUniform};

#[cfg(target_arch = "wasm32")]
type Rc<T> = std::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
type Rc<T> = std::sync::Arc<T>;

#[cfg(target_arch = "wasm32")]
const CANVAS_ID: &str = "wgpu-canvas";

pub fn create_graphics(event_loop: &ActiveEventLoop) -> impl Future<Output = Renderer> + 'static {
    #[allow(unused_mut)]
    let mut window_attrs = Window::default_attributes();

    #[cfg(target_arch = "wasm32")]
    {
        use web_sys::wasm_bindgen::JsCast;
        use winit::platform::web::WindowAttributesExtWebSys;

        let window = web_sys::window().unwrap_throw();
        let document = window.document().unwrap_throw();
        let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
        let html_canvas_element = canvas.unchecked_into();
        window_attrs = window_attrs.with_canvas(Some(html_canvas_element));
    }

    let window = Rc::new(event_loop.create_window(window_attrs).unwrap_throw());
    let instance = wgpu::Instance::default();
    let surface = instance
        .create_surface(window.clone())
        .unwrap_or_else(|e| throw_str(&format!("{e:#?}")));

    async move {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
            })
            .await
            .unwrap_throw();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .unwrap_throw();

        let size = window.inner_size();
        let surface_config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap_throw();

        #[cfg(not(target_arch = "wasm32"))]
        {
            surface.configure(&device, &surface_config);
        }

        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 1.0),
            target: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            aspect: size.width as f32 / size.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        let camera_controller = CameraController::new(0.2, 0.2);
        let mut camera_uniform = CameraUniform::from_camera(&camera);
        let camera_bind_group_layout = CameraUniform::get_bind_group_layout(&device);
        let camera_buffer = camera_uniform.get_camera_buffer(&device);
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let scene_graph = create_scenegraph();
        let buffer_wrappers = create_buffers_from_scenegraph(&device, &camera, &scene_graph);

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        Renderer {
            window,
            instance,
            surface,
            surface_config,
            adapter,
            device,
            queue,
            render_pipeline,
            buffer_wrappers,
            camera,
            camera_controller,
            camera_buffer,
            camera_bind_group,
            camera_uniform,
            scene_graph,
        }
    }
}

fn create_buffers_from_scenegraph(device: &Device, camera: &Camera, scene_graph: &SceneGraph) -> Vec<BufferWrapper> {
    let scene_graph_iter = SceneGraphIterator::new(&scene_graph, camera.calculate_matrix());
    let mut buffer_wrappers = vec![];
    for node in scene_graph_iter {
        println!("{:?}", node.vertices);
        print!("____");
        let num_vertices = node.vertices.len() as u32;
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(node.get_name().as_str()),
            contents: bytemuck::cast_slice(&*node.vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let num_indices = node.indices.len() as u32;
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(node.get_name().as_str()),
            contents: bytemuck::cast_slice(&*node.indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        buffer_wrappers.push(BufferWrapper {
            vertex_buffer,
            index_buffer,
            num_indices,
            num_vertices,
        });
    }
    buffer_wrappers
}

pub fn create_scenegraph() -> SceneGraph {
    let mut scenegraph = SceneGraph::new();
    let mut triangle = RenderNode::new("triangle".to_string());
    triangle.set_vertices(TEST_VERTICES.to_vec());
    let matrix = glam::Mat4::from_scale_rotation_translation(
        glam::Vec3::new(1.0, 1.0, 1.0),
        glam::Quat::from_rotation_z(0.0),
        glam::Vec3::new(0.5, 0.0, 0.0),
    );
    triangle.set_matrix(matrix);
    scenegraph.add_child_to_root(Node::RenderNode(triangle));
    scenegraph
}

#[derive(Clone)]
pub struct BufferWrapper {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub num_vertices: u32,
}

pub struct Renderer {
    window: Rc<Window>,
    instance: Instance,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub render_pipeline: RenderPipeline,
    pub buffer_wrappers: Vec<BufferWrapper>,
    scene_graph: SceneGraph,
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.camera.resize(size.width as f32, size.height as f32);
    }

    pub fn update_buffers(&mut self) {
        let scene_graph_iter = SceneGraphIterator::new(&self.scene_graph, self.camera.calculate_matrix());
        let mut scene_graph_count = 0;

        // TODO this might cause problems
        for (i, node) in scene_graph_iter.enumerate() {
            scene_graph_count += 1;
            if i >= self.buffer_wrappers.len() {
                // If the scene graph has more nodes than our current buffers, create new ones
                let num_vertices = node.vertices.len() as u32;
                let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(node.get_name().as_str()),
                    contents: bytemuck::cast_slice(&*node.vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });
                let num_indices = node.indices.len() as u32;
                let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(node.get_name().as_str()),
                    contents: bytemuck::cast_slice(&*node.indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });
                self.buffer_wrappers.push(BufferWrapper {
                    vertex_buffer,
                    index_buffer,
                    num_indices,
                    num_vertices,
                });
            } else {
                // Update existing buffers using queue.write_buffer
                self.queue.write_buffer(
                    &self.buffer_wrappers[i].vertex_buffer,
                    0,
                    bytemuck::cast_slice(&*node.vertices)
                );

                self.queue.write_buffer(
                    &self.buffer_wrappers[i].index_buffer,
                    0,
                    bytemuck::cast_slice(&*node.indices)
                );

                // Update counts in case they changed
                self.buffer_wrappers[i].num_vertices = node.vertices.len() as u32;
                self.buffer_wrappers[i].num_indices = node.indices.len() as u32;
            }
        }

        // Remove any extra buffers if the scene graph has fewer nodes now
        if self.buffer_wrappers.len() > scene_graph_count {
            self.buffer_wrappers.truncate(scene_graph_count);
        }
    }
}

pub struct RenderProxy {
    event_loop_proxy: Option<EventLoopProxy<Renderer>>,
}

impl RenderProxy {
    pub fn new(event_loop_proxy: EventLoopProxy<Renderer>) -> Self {
        Self {
            event_loop_proxy: Some(event_loop_proxy),
        }
    }

    pub fn build_and_send(&mut self, event_loop: &ActiveEventLoop) {
        let Some(event_loop_proxy) = self.event_loop_proxy.take() else {
            // event_loop_proxy is already spent - we already constructed Graphics
            return;
        };

        #[cfg(target_arch = "wasm32")]
        {
            let gfx_fut = create_graphics(event_loop);
            wasm_bindgen_futures::spawn_local(async move {
                let gfx = gfx_fut.await;
                assert!(event_loop_proxy.send_event(gfx).is_ok());
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let gfx = pollster::block_on(create_graphics(event_loop));
            assert!(event_loop_proxy.send_event(gfx).is_ok());
        }
    }
}
