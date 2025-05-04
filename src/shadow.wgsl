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

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) depth: f32,
};

@group(1) @binding(0)
var<uniform> model: Model;

@vertex
fn vs_shadow(in: VertexInput) -> VertexOutput {
    let world_pos = model.model * vec4<f32>(in.position, 1.0);
    let view_pos = camera.view_proj * world_pos;

    var out: VertexOutput;
    out.position = view_pos;
    out.depth = view_pos.z / view_pos.w;
    return out;
}

fn get_optimized_moments(depth: f32) -> vec4<f32> {
    // the moments, exponentials of the depth
    let square = depth * depth;
    let moments = vec4<f32>(depth, square, square * depth, square * square);

    let m = mat4x4<f32>(
        vec4<f32>(-2.07224649,    13.7948857237,  0.105877704,   9.7924062118),
        vec4<f32>( 32.23703778,  -59.4683975703, -1.9077466311, -33.7652110555),
        vec4<f32>(-68.571074599,  82.0359750338,  9.3496555107,  47.9456096605),
        vec4<f32>( 39.3703274134, -35.364903257,  -6.6543490743, -23.9728048165)
    );

    var optimized = m * moments;
    optimized.x += 0.035955884801;
    return optimized;
}


@fragment
fn fs_shadow(@location(0) z: f32) -> @location(0) vec4<f32> {
    return get_optimized_moments(z);
}