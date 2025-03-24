mod application;
mod renderer;
mod scenegraph;
mod camera;
mod model;
mod resources;
mod texture;

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::UnwrapThrowExt;
use crate::application::{App};
use winit::event_loop::{ControlFlow, EventLoop};


fn main() {
    let event_loop = EventLoop::with_user_event().build().unwrap();
    let mut app = App::new(&event_loop);

    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).expect("Failed to run app");
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run_web() {
    let window = web_sys::window().unwrap_throw();
    let document = window.document().unwrap_throw();

    let canvas = document.create_element("canvas").unwrap_throw();
    canvas.set_id(crate::application::CANVAS_ID);
    canvas.set_attribute("width", "500").unwrap_throw();
    canvas.set_attribute("height", "500").unwrap_throw();

    let body = document
        .get_elements_by_tag_name("body")
        .item(0)
        .unwrap_throw();
    body.append_with_node_1(canvas.unchecked_ref())
        .unwrap_throw();

    run();
}