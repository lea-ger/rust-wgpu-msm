/*
 * This setup was taken from https://github.com/erer1243/wgpu-0.20-winit-0.30-web-example/blob/master/src/lib.rs.
 * Reason is that it's tricky to set up a WGPU pipeline using the latest version of WGPU and Winit, especially when targeting the web.
 *
 */
use crate::light::ShadowMap;
use crate::renderer::{RenderProxy, Renderer};
use crate::scenegraph::{DrawScenegraph, SceneGraphLightNodeIterator};
use crate::texture::Texture;
#[allow(unused_imports)]
use wasm_bindgen::{prelude::wasm_bindgen, throw_str, JsCast, UnwrapThrowExt};
use wgpu::hal::DynCommandEncoder;
use wgpu::util::RenderEncoder;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

enum MaybeRenderer {
    Proxy(RenderProxy),
    Renderer(Renderer),
}

pub struct App {
    renderer: MaybeRenderer,
    last_render_time: instant::Instant,
}

impl App {
    pub fn new(event_loop: &EventLoop<Renderer>) -> Self {
        Self {
            renderer: MaybeRenderer::Proxy(RenderProxy::new(event_loop.create_proxy())),
            last_render_time: instant::Instant::now(),
        }
    }

    fn draw(&mut self) {
        let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
            return;
        };

        let frame = renderer.surface.get_current_texture().unwrap_throw();
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.device.create_command_encoder(&Default::default());

        // 1) Alignment‑Konstante holen
        const ALIGN: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

        // 2) Größe des Buffers (ein Row, eine Zeile)
        let buffer_size  = ALIGN as u64 * 1 /* height */;

        let readback = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let now = instant::Instant::now();
        let dt = now - self.last_render_time;
        self.last_render_time = now;

        // shadow pass
        render_shadow_pass(renderer, &mut encoder);

        renderer
            .camera_state
            .camera_controller
            .update_camera(&mut renderer.camera_state.camera);
        renderer
            .camera_state
            .camera_uniform
            .update(&renderer.camera_state.camera);
        renderer.queue.write_buffer(
            &renderer.camera_state.camera_buffer,
            0,
            bytemuck::cast_slice(&[renderer.camera_state.camera_uniform]),
        );

        // forward pass
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            rpass.set_pipeline(&renderer.render_pipeline.pipeline);
            rpass.set_bind_group(0, &renderer.camera_state.camera_bind_group, &[]);
            rpass.set_bind_group(1, &renderer.model_matrix_bind_group, &[]);
            rpass.set_bind_group(3, &renderer.scene_graph.light_bind_group, &[]);
            rpass.draw_scenegraph(
                &renderer.scene_graph,
                &renderer.queue,
                2,
                &renderer.model_matrix_buffer,
                &renderer.camera_state.camera.eye,
            );
        }

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &renderer.scene_graph.shadow_map.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 }, // Array‑Slice 0
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(ALIGN),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        renderer.queue.submit(Some(encoder.finish()));
        frame.present();
        let buffer_slice = readback.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| ());
        renderer.device.poll(wgpu::Maintain::Wait);

        let data = buffer_slice.get_mapped_range();
        let depth_value = bytemuck::from_bytes::<f32>(&data[0..4]);
        println!("ShadowMap[0,0] = {:?}", depth_value);

        renderer.scene_graph.on_frame_update();
    }

    fn resized(&mut self, size: PhysicalSize<u32>) {
        let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
            return;
        };
        renderer.surface_config.width = size.width;
        renderer.surface_config.height = size.height;
        renderer
            .surface
            .configure(&renderer.device, &renderer.surface_config);
        renderer
            .camera_state
            .camera
            .resize(size.width as f32, size.height as f32);

        renderer.depth_texture = Texture::create_depth_texture(
            &renderer.device,
            &renderer.surface_config,
            "depth_texture",
        );
    }
}

fn render_shadow_pass(renderer: &Renderer, encoder: &mut wgpu::CommandEncoder) {
    let scene_graph = &renderer.scene_graph;

    for light_node in SceneGraphLightNodeIterator::new(&renderer.scene_graph) {
        let light = &light_node.0.light;
        let model = light_node.1;
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &light.target_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        rpass.set_pipeline(&renderer.shadow_pipeline.pipeline);
        rpass.set_bind_group(1, &renderer.model_matrix_bind_group, &[]);

        let mut temp_camera_uniform = light.to_camera_uniform(model);
        renderer.queue.write_buffer(
            &renderer.camera_state.camera_buffer,
            0,
            bytemuck::cast_slice(&[temp_camera_uniform]),
        );
        rpass.set_bind_group(0, &renderer.camera_state.camera_bind_group, &[]);

        rpass.draw_scenegraph_vertices(scene_graph, &renderer.queue, &renderer.model_matrix_buffer);
    }
}

impl ApplicationHandler<Renderer> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let MaybeRenderer::Proxy(builder) = &mut self.renderer {
            builder.build_and_send(event_loop);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, graphics: Renderer) {
        self.renderer = MaybeRenderer::Renderer(graphics);
    }

    fn device_event(
        &mut self,
        _: &ActiveEventLoop,
        _: DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => {}
            _ => (),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::KeyboardInput { .. } => {
                if let MaybeRenderer::Renderer(renderer) = &mut self.renderer {
                    let state_changed = renderer
                        .camera_state
                        .camera_controller
                        .process_events(&event);
                    if state_changed {
                        self.draw();
                    }
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                ..
            } => {
                if let MaybeRenderer::Renderer(renderer) = &mut self.renderer {
                    let state_changed = renderer
                        .camera_state
                        .camera_controller
                        .process_events(&event);
                    if state_changed {
                        self.draw();
                    }
                }
            }
            WindowEvent::CursorMoved {
                ..
            } => {
                if let MaybeRenderer::Renderer(renderer) = &mut self.renderer {
                    let state_changed = renderer
                        .camera_state
                        .camera_controller
                        .process_events(&event);
                    if state_changed {
                        self.draw();
                    }
                }
            }
            _ => (),
        }
    }
}
