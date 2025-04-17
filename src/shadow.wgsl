struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
};

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct Model {
    model: mat4x4<f32>,
};

@group(1) @binding(0)
var<uniform> model: Model;

@vertex
fn vs_shadow(@location(0) position: vec4<f32>) -> @builtin(position) vec4<f32> {
    let world = model.model;

    return camera.view_proj * world * vec4<f32>(position);
}
