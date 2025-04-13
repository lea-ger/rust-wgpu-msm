use crate::camera::{Camera, CameraController, CameraUniform};
use crate::light::Light;
use crate::model::{load_model, Material, Mesh, Model, Vertex, TEST_INDICES, TEST_VERTICES};
use crate::scenegraph::{GroupNode, ModelUniform, Node, RenderNode, SceneGraph, SceneGraphRenderNodeIterator};
use crate::texture::get_default_texture;
use crate::{light, texture};
use glam::{Mat4, Vec3};
use std::borrow::Cow;
use std::future::Future;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use wasm_bindgen::{throw_str, UnwrapThrowExt};
use wgpu::util::DeviceExt;
use wgpu::{
    Adapter, BindGroupLayout, Device, Instance, Queue, RenderPipeline, Surface,
    SurfaceConfiguration,
};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::Window;

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
    let required_features = wgpu::Features::BUFFER_BINDING_ARRAY | wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY;

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
                    required_features,
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
        let surface_config = SurfaceConfiguration {
            ..surface
                .get_default_config(&adapter, size.width, size.height)
                .unwrap_throw()
        };

        let supports_storage_resources = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::VERTEX_STORAGE)
            && device.limits().max_storage_buffers_per_shader_stage > 0;

        #[cfg(not(target_arch = "wasm32"))]
        {
            surface.configure(&device, &surface_config);
        }

        let camera = Camera {
            eye: Vec3::new(0.0, 1.0, 50.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            aspect: size.width as f32 / size.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.,
        };
        let camera_controller = CameraController::new(1., 0.2);
        let mut camera_uniform = CameraUniform::from_camera(&camera);
        let camera_bind_group_layout = CameraUniform::get_bind_group_layout(&device);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("material_bind_group_layout"),
            });

        let model_matrix_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("model_matrix_bind_group_layout"),
        });
        let model_matrix_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Model Matrix Buffer"),
            size: size_of::<ModelUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let model_matrix_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &model_matrix_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: model_matrix_buffer.as_entire_binding(),
            }],
            label: Some("model_matrix_bind_group"),
        });

        let scene_graph = create_scenegraph(
            &device,
            &queue,
            &material_bind_group_layout,
            supports_storage_resources,
        )
        .await;

        let light_bind_group_layout = &scene_graph.light_bind_group_layout;
        let bind_group_layouts: Vec<&BindGroupLayout> = {
            let mut layouts = vec![&camera_bind_group_layout, &material_bind_group_layout, &model_matrix_bind_group_layout];
            if let Some(ref light_layout) = light_bind_group_layout {
                layouts.push(light_layout);
            }
            layouts
        };

        println!("layouts: {:?}", bind_group_layouts.len());

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &surface_config, "depth_texture");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &bind_group_layouts,
            push_constant_ranges: &[],
        });
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
                entry_point: Some(if supports_storage_resources {
                    "fs_main"
                } else {
                    "fs_main_without_storage"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
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
            camera,
            camera_controller,
            camera_buffer,
            camera_bind_group,
            camera_uniform,
            scene_graph,
            depth_texture,
            model_matrix_buffer,
            model_matrix_bind_group
        }
    }
}

pub async fn create_scenegraph(
    device: &Device,
    queue: &Queue,
    material_bind_group_layout: &BindGroupLayout,
    supports_storage_resources: bool,
) -> SceneGraph {
    let mut scenegraph = SceneGraph::new(supports_storage_resources);

    let ground_vertices = [
        Vertex {
            tex_coords: [-1.0, -1.0],
            pos: [-50.0, 0.0, -50.0],
            normal: [0.0, 1.0, 0.0],
        },
        Vertex {
            tex_coords: [-1.0, -1.0],
            pos: [50.0, 0.0, -50.0],
            normal: [0.0, 1.0, 0.0],
        },
        Vertex {
            tex_coords: [-1.0, -1.0],
            pos: [50.0, 0.0, 50.0],
            normal: [0.0, 1.0, 0.0],
        },
        Vertex {
            tex_coords: [-1.0, -1.0],
            pos: [-50.0, 0.0, 50.0],
            normal: [0.0, 1.0, 0.0],
        },
    ];
    let ground_indices = [0, 1, 2, 0, 2, 3];
    let default_texture =
        texture::Texture::from_image(device, queue, &get_default_texture(), Some("ground"))
            .unwrap_or_else(|e| throw_str(&format!("{e:#?}")));
    let ground = Model {
        meshes: vec![Mesh {
            name: "ground".to_string(),
            vertices: ground_vertices.to_vec(),
            indices: ground_indices.to_vec(),
            material: 0,
            num_elements: ground_indices.len() as u32,
        }],
        materials: vec![Material {
            name: "ground".to_string(),
            diffuse_texture: Some(default_texture),
            material: tobj::Material {
                name: "ground".to_string(),
                diffuse: Some([0.4, 0.3, 0.2]),
                dissolve: Some(1.0),
                ..Default::default()
            },
        }],
    };
    scenegraph.add_model_node(
        None,
        "ground".to_string(),
        device,
        &ground,
        material_bind_group_layout,
        Mat4::IDENTITY,
    );

    let model = load_model("assets/All_Files/Example/OBJ", "Example.obj", device, queue);
    scenegraph.add_model_node(
        None,
        "house".to_string(),
        device,
        &model.await.unwrap(),
        material_bind_group_layout,
        Mat4::IDENTITY,
    );
    scenegraph.add_light_node(
        None,
        "light".to_string(),
        device,
        Light::new(
            Vec3::new(10.0, 5.0, 0.0),
            wgpu::Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        ),
    );
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
    pub scene_graph: SceneGraph,
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub depth_texture: texture::Texture,
    pub model_matrix_buffer: wgpu::Buffer,
    pub model_matrix_bind_group: wgpu::BindGroup,
}

impl Renderer {
    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        self.surface.configure(&self.device, &self.surface_config);
        self.camera.resize(size.width as f32, size.height as f32);
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
