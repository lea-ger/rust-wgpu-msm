use crate::camera::{Camera, CameraController, CameraUniform};
use crate::light::{Light, ShadowMap};
use crate::model::{load_model, Material, Mesh, Model, Vertex, CUBE_INDICES, CUBE_VERTICES};
use crate::scenegraph::{
    GroupNode, ModelUniform, Node, RenderNode, SceneGraph, SceneGraphRenderNodeIterator,
};
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
use winit::window::{Window, WindowAttributes};

#[cfg(target_arch = "wasm32")]
type Rc<T> = std::rc::Rc<T>;

#[cfg(not(target_arch = "wasm32"))]
type Rc<T> = std::sync::Arc<T>;

#[cfg(target_arch = "wasm32")]
const CANVAS_ID: &str = "wgpu-canvas";

pub struct Pipeline {
    pub layout: wgpu::PipelineLayout,
    pub pipeline: wgpu::RenderPipeline,
}

impl Pipeline {
    pub fn new(
        device: &Device,
        shader: &wgpu::ShaderModule,
        bind_group_layouts: &[&BindGroupLayout],
        vertex_entry: &str,
        vertex_buffer_layout: &[wgpu::VertexBufferLayout],
        fragment_entry: Option<&str>,
        color_target: &[Option<wgpu::ColorTargetState>],
        depth_format: Option<wgpu::TextureFormat>,
        depth_bias: Option<wgpu::DepthBiasState>,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts,
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some(vertex_entry),
                compilation_options: Default::default(),
                buffers: vertex_buffer_layout,
            },
            fragment: if let Some(entry) = fragment_entry {
                Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: color_target,
                })
            } else {
                None
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: device
                    .features()
                    .contains(wgpu::Features::DEPTH_CLIP_CONTROL),
                conservative: false,
                ..Default::default()
            },
            depth_stencil: if let Some(format) = depth_format {
                Some(wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: depth_bias.unwrap_or_default(),
                })
            } else {
                None
            },
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            layout,
            pipeline,
        }
    }
}

pub struct Renderer {
    pub window: Rc<Window>,
    instance: Instance,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
    pub render_pipeline: Pipeline,
    pub shadow_pipeline: Pipeline,
    pub scene_graph: SceneGraph,
    pub depth_texture: texture::Texture,
    pub model_matrix_buffer: wgpu::Buffer,
    pub model_matrix_bind_group: wgpu::BindGroup,
    pub camera_state: CameraState,
    pub sp_camera_buffer: wgpu::Buffer,
    pub sp_camera_bind_group: wgpu::BindGroup,
}

pub struct CameraState {
    pub camera: Camera,
    pub camera_controller: CameraController,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
}

pub fn create_graphics(event_loop: &ActiveEventLoop) -> impl Future<Output = Renderer> + 'static {
    #[allow(unused_mut)]
    let mut window_attrs = Window::default_attributes();
    window_attrs.maximized = true;

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
    let required_features =
        wgpu::Features::BUFFER_BINDING_ARRAY | wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY;

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
            eye: Vec3::new(0.0, 1.0, 30.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            aspect: size.width as f32 / size.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.,
        };
        let camera_controller = CameraController::new(0.5, 0.1);
        let camera_uniform = CameraUniform::from_camera(&camera);
        let camera_bind_group_layout = CameraUniform::get_bind_group_layout(&device);
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let sp_camera_bind_group_layout = CameraUniform::get_bind_group_layout(&device);
        let sp_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: size_of::<CameraUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sp_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &sp_camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sp_camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let camera_state = CameraState {
            camera,
            camera_controller,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
        };

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shadow.wgsl"))),
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

        let model_matrix_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let shadow_map = ShadowMap::create_shadow_map(&device);

        let scene_graph = create_scenegraph(
            &device,
            &queue,
            &material_bind_group_layout,
            supports_storage_resources,
            shadow_map
        )
        .await;

        let light_bind_group_layout = &scene_graph.light_bind_group_layout;

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &surface_config, "depth_texture");
        let vertex_buffer_layout = Vertex::desc();

        let shadow_pipeline = Pipeline::new(
            &device,
            &shadow_shader,
            &[&sp_camera_bind_group_layout, &model_matrix_bind_group_layout],
            "vs_shadow",
            &[vertex_buffer_layout.clone()],
            None,
            &[None],
            Some(ShadowMap::DEPTH_FORMAT),
            Some(wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            }),
        );

        let render_pipeline = Pipeline::new(
            &device,
            &shader,
            &[
                &camera_bind_group_layout,
                &model_matrix_bind_group_layout,
                &material_bind_group_layout,
                light_bind_group_layout.as_ref().unwrap(),
            ],
            "vs_main",
            &[vertex_buffer_layout],
            Some(if supports_storage_resources {
                "fs_main"
            } else {
                "fs_main_without_storage"
            }),
            &[Some(wgpu::ColorTargetState {
                format: surface_config.format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Max,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            Some(texture::Texture::DEPTH_FORMAT),
            None,
        );

        Renderer {
            window,
            instance,
            surface,
            surface_config,
            adapter,
            device,
            queue,
            render_pipeline,
            shadow_pipeline,
            scene_graph,
            depth_texture,
            model_matrix_buffer,
            model_matrix_bind_group,
            camera_state,
            sp_camera_buffer,
            sp_camera_bind_group,
        }
    }
}

pub async fn create_scenegraph(
    device: &Device,
    queue: &Queue,
    material_bind_group_layout: &BindGroupLayout,
    supports_storage_resources: bool,
    shadow_map: ShadowMap
) -> SceneGraph {
    let light_pos = Vec3::new(0.0, 25.0, 30.0);
    let light_sun = Light::new(
        light_pos,
        wgpu::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        &shadow_map.texture,
        0,
    );
    let light_cube_vertices = CUBE_VERTICES.iter().map(|vertex| {
        Vertex {
            tex_coords: vertex.tex_coords,
            pos: [
                vertex.pos[0] * 0.1,
                vertex.pos[1] * 0.1,
                vertex.pos[2] * 0.1,
            ],
            normal: vertex.normal,
        }
    }).collect::<Vec<_>>();

    let light_sun_model = Model {
        meshes: vec![Mesh {
            name: "light".to_string(),
            vertices: light_cube_vertices.to_vec(),
            indices: CUBE_INDICES.to_vec(),
            material: 0,
            num_elements: CUBE_INDICES.len() as u32,
        }],
        materials: vec![Material::new(
            "light",
            Some([1.0, 1.0, 0.0]),
            device,
            queue,
        )],
    };

    let mut scenegraph = SceneGraph::new(supports_storage_resources, shadow_map);

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
        materials: vec![Material::new("ground", Some([0.4, 0.3, 0.2]), device, queue)],
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
    scenegraph.add_light_node(None, "light".to_string(), device, light_sun);
    scenegraph.add_model_node(
        None,
        "light_model".to_string(),
        device,
        &light_sun_model,
        material_bind_group_layout,
        Mat4::from_translation(light_pos),
    );
    scenegraph
}

pub fn rotate_sun(device: &Device, scene_graph: &mut SceneGraph, time: f32) {
    let mut pos;
    {
        let node = scene_graph.find_child_mut(Some("light")).unwrap();
        let light_node = match node {
            Node::LightNode(light) => light,
            _ => panic!("Expected a light node"),
        };
        let light = &mut light_node.light;

        let radius = 30.0;
        let speed = 0.5;
        let angle = time * speed as f32 * std::f32::consts::PI / 2.0;

        light.pos.x = radius * angle.cos();
        light.pos.z = radius * angle.sin();
        pos = light.pos;
    }

    {
        let model_node = scene_graph.find_child_mut(Some("light_model-light"));
        if model_node.is_none() {
            return;
        }
        let model_node = model_node.unwrap();
        let light_model_node = match model_node {
            Node::RenderNode(render) => render,
            _ => return
        };

        light_model_node.set_matrix(Mat4::from_translation(pos), device);
    };

    scene_graph.update_light_bind_group(device);
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
