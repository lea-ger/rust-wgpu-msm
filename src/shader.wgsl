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
struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(2) @binding(0)
var<storage, read> s_lights: array<Light>;
@group(2) @binding(0)
var<uniform> u_lights: array<Light, 10>;

@vertex
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    return VertexOutput(
        camera.view_proj * vec4<f32>(in.position, 1.0),
        in.tex_coords
    );
}

struct Material {
    ambient: vec4<f32>,
    diffuse: vec4<f32>,
    specular: vec4<f32>,
    shininess: f32,
    dissolve: f32,
};

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;
@group(1) @binding(2)
var<uniform> material: Material;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var texture_result = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    if (texture_result.r == 0.0 && texture_result.g == 0.0 && texture_result.b == 0.0 && texture_result.a == 0.0) {
        return vec4<f32>(
            material.diffuse.r,
            material.diffuse.g,
            material.diffuse.b,
            material.dissolve
        );
    }

    return texture_result;
}