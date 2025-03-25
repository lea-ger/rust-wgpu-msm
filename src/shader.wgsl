struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

struct Camera {
    view_proj: mat4x4<f32>,
    position: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: Camera;

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    return VertexOutput(
        camera.view_proj * vec4<f32>(in.position, 1.0),
        in.tex_coords
    );
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}