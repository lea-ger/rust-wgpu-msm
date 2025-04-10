/*
 * This setup was taken from https://github.com/erer1243/wgpu-0.20-winit-0.30-web-example/blob/master/src/lib.rs.
 * Reason is that it's tricky to set up a WGPU pipeline using the latest version of WGPU and Winit, especially when targeting the web.
 *
 */
use crate::renderer::{RenderProxy, Renderer};
use crate::scenegraph::DrawScenegraph;
#[allow(unused_imports)]
use wasm_bindgen::{prelude::wasm_bindgen, throw_str, JsCast, UnwrapThrowExt};
use wgpu::util::{DeviceExt, RenderEncoder};
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};
use crate::light::LightUniform;
use crate::texture::Texture;

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
            last_render_time: instant::Instant::now()
        }
    }

    fn draw(&mut self) {
        let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
            return;
        };

        let frame = renderer.surface.get_current_texture().unwrap_throw();
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.device.create_command_encoder(&Default::default());

        let now = instant::Instant::now();
        let dt = now - self.last_render_time;
        self.last_render_time = now;
        renderer.camera_controller.update_camera(&mut renderer.camera);
        renderer.camera_uniform.update(&renderer.camera);
        renderer.queue.write_buffer(&renderer.camera_buffer, 0, bytemuck::cast_slice(&[renderer.camera_uniform]));

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
                },
                )],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &renderer.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            rpass.set_pipeline(&renderer.render_pipeline);
            rpass.set_bind_group(0, &renderer.camera_bind_group, &[]);
            rpass.draw_scenegraph(&renderer.scene_graph, 1, 2, &renderer.camera.eye);
        }

        let command_buffer = encoder.finish();
        renderer.queue.submit([command_buffer]);
        frame.present();
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

        renderer.depth_texture = Texture::create_depth_texture(&renderer.device, &renderer.surface_config, "depth_texture");
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
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        match event {
            DeviceEvent::MouseMotion { delta } => {
            },
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
                    renderer.camera_controller.process_events(&event);
                    self.draw();
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                if let MaybeRenderer::Renderer(renderer) = &mut self.renderer {
                    renderer.camera_controller.process_events(&event);
                    self.draw();
                }
            }
            WindowEvent::CursorMoved {
                position,
                device_id,
                ..
            } => {
                if let MaybeRenderer::Renderer(renderer) = &mut self.renderer {
                    renderer.camera_controller.process_events(&event);
                    self.draw();
                }
            }
            _ => (),
        }
    }
}
