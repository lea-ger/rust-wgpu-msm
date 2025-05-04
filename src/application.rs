/*
 * This setup was taken from https://github.com/erer1243/wgpu-0.20-winit-0.30-web-example/blob/master/src/lib.rs.
 * Reason is that it's tricky to set up a WGPU pipeline using the latest version of WGPU and Winit, especially when targeting the web.
 *
 */
use crate::renderer::{rotate_sun, RenderProxy, Renderer};
use crate::scenegraph::{DrawScenegraph, SceneGraphLightNodeIterator};
use crate::texture::Texture;
use std::time::{Duration, Instant};
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

pub enum MaybeRenderer {
    Proxy(RenderProxy),
    Renderer(Renderer),
}

pub struct App {
    pub renderer: MaybeRenderer,
    start_time: instant::Instant,
    shadow_pass_debug_camera_bind_group: Option<wgpu::BindGroup>,
    target_frame_time: Duration,
}

impl App {
    pub fn new(event_loop: &EventLoop<Renderer>) -> Self {
        Self {
            renderer: MaybeRenderer::Proxy(RenderProxy::new(event_loop.create_proxy())),
            start_time: Instant::now(),
            shadow_pass_debug_camera_bind_group: None,
            target_frame_time: Duration::from_secs_f64(1.0 / 60.0),
        }
    }

    pub fn draw(&mut self) {
        let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
            return;
        };

        let frame = renderer.surface.get_current_texture().unwrap_throw();
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.device.create_command_encoder(&Default::default());

        let now = Instant::now();

        rotate_sun(&renderer.device, &mut renderer.scene_graph, (now - self.start_time).as_secs_f32());

        // shadow pass
        {
            render_shadow_pass(renderer, &mut encoder);
        }

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

        renderer.queue.submit(Some(encoder.finish()));
        frame.present();

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

        let temp_camera_uniform = light.to_camera_uniform(model);
        renderer.queue.write_buffer(
            &renderer.sp_camera_buffer,
            0,
            bytemuck::cast_slice(&[temp_camera_uniform]),
        );
        rpass.set_bind_group(0, &renderer.sp_camera_bind_group, &[]);

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

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => {
                let frame_start = Instant::now();

                self.draw();

                let elapsed = frame_start.elapsed();
                if elapsed < self.target_frame_time {
                    let wait_duration = self.target_frame_time - elapsed;
                    std::thread::sleep(wait_duration);
                }

                let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
                    return;
                };
                renderer.window.request_redraw();
            },
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
}
