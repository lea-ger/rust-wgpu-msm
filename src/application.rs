/*
 * This setup was taken from https://github.com/erer1243/wgpu-0.20-winit-0.30-web-example/blob/master/src/lib.rs.
 * Reason is that it's tricky to set up a WGPU pipeline using the latest version of WGPU and Winit, especially when targeting the web.
 *
 */

#[allow(unused_imports)]
use wasm_bindgen::{prelude::wasm_bindgen, throw_str, JsCast, UnwrapThrowExt};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};
use crate::renderer::{RenderProxy, Renderer};

enum MaybeRenderer {
    Proxy(RenderProxy),
    Renderer(Renderer),
}

pub struct App {
    renderer: MaybeRenderer,
}

impl App {
    pub fn new(event_loop: &EventLoop<Renderer>) -> Self {
        Self {
            renderer: MaybeRenderer::Proxy(RenderProxy::new(event_loop.create_proxy())),
        }
    }

    fn draw(&mut self) {
        let MaybeRenderer::Renderer(renderer) = &mut self.renderer else {
            return;
        };

        let frame = renderer.surface.get_current_texture().unwrap_throw();
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = renderer.device.create_command_encoder(&Default::default());

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            rpass.set_pipeline(&renderer.render_pipeline);
            rpass.set_vertex_buffer(0, renderer.vertex_buffer.slice(..));
            rpass.draw(0..renderer.num_vertices, 0..1);
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
        renderer.surface.configure(&renderer.device, &renderer.surface_config);
    }
}

impl ApplicationHandler<Renderer> for App {
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(size) => self.resized(size),
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let MaybeRenderer::Proxy(builder) = &mut self.renderer {
            builder.build_and_send(event_loop);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, graphics: Renderer) {
        self.renderer = MaybeRenderer::Renderer(graphics);
    }
}