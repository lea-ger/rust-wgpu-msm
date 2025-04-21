struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) out_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec4<f32>,
    @location(2) world_normal: vec3<f32>,
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
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    let world = model.model;
    let world_position = world * vec4<f32>(in.position, 1.0);

    return VertexOutput(
        camera.view_proj * world_position,
        in.tex_coords,
        world_position,
        mat3x3<f32>(world[0].xyz, world[1].xyz, world[2].xyz) * in.normal,
    );
}

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
    model: mat4x4<f32>,
    view_proj: mat4x4<f32>,
}
@group(3) @binding(0)
var<storage, read> s_lights: array<Light>;
@group(3) @binding(0)
var<uniform> u_lights: array<Light, 10>;
@group(3) @binding(1) var t_shadow: texture_depth_2d_array;
@group(3) @binding(2) var sampler_shadow: sampler_comparison;

fn fetch_shadow(light_id: u32, ls_pos: vec4<f32>) -> f32 {
    // compensate for the Y-flip difference between the NDC and texture coordinates
    let flip_correction = vec2<f32>(0.5, -0.5);
    // compute texture coordinates for shadow lookup
    let proj_correction = 1.0 / ls_pos.w;
    let light_local = ls_pos.xy * flip_correction * proj_correction + vec2<f32>(0.5, 0.5);
    // do the lookup, using HW PCF and comparison
    let closest_depth = textureSampleCompareLevel(t_shadow, sampler_shadow, light_local, i32(light_id), ls_pos.z * proj_correction);
    // compare the depth of the fragment with the closest depth
    if (closest_depth < ls_pos.z) {
        return 0.0;
    }
    return 1.0;
}

struct Material {
    ambient: vec4<f32>,
    diffuse: vec4<f32>,
    specular: vec4<f32>,
    shininess: f32,
    dissolve: f32,
};

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var<uniform> material: Material;

fn phong (light: Light, normal: vec3<f32>, in: VertexOutput, shadow: f32) -> vec3<f32> {
    let light_world_position = light.model * vec4<f32>(light.position, 1.0);
    let light_dir = normalize(light_world_position.xyz - in.out_position.xyz);

    let diffuse = max(0.0, dot(normal, light_dir));

    let view_dir = normalize(camera.position.xyz - in.out_position.xyz);
    let reflect_dir  = reflect(-light_dir, in.world_normal);
    let specular = pow(max(0.0, dot(normal, reflect_dir)), (10 * material.shininess));

    return (diffuse * light.color.xyz + specular * material.specular.xyz) * shadow;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var texture_result = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    var material_color = texture_result;
    let normal = normalize(in.world_normal);

    if (texture_result.r == 0.0 && texture_result.g == 0.0 && texture_result.b == 0.0 && texture_result.a == 0.0) {
        material_color = vec4<f32>(
            material.diffuse.r,
            material.diffuse.g,
            material.diffuse.b,
            material.dissolve
        );
    }
    var light_color: vec3<f32> = vec3<f32>(0.3, 0.3, 0.3);
    for (var i = 0u; i < 3; i += 1u) {
        let light = s_lights[i];
        let shadow = fetch_shadow(i, light.view_proj * in.world_position);

        light_color += phong(light, normal, in, shadow);
    }

    return vec4<f32>(light_color, 1.0) * material_color;
}

@fragment
fn fs_main_without_storage(in: VertexOutput) -> @location(0) vec4<f32> {
    var texture_result = textureSample(t_diffuse, s_diffuse, in.tex_coords);
        var material_color = texture_result;
        let normal = normalize(in.world_normal);

        if (texture_result.r == 0.0 && texture_result.g == 0.0 && texture_result.b == 0.0 && texture_result.a == 0.0) {
            material_color = vec4<f32>(
                material.diffuse.r,
                material.diffuse.g,
                material.diffuse.b,
                material.dissolve
            );
        }
        var light_color: vec3<f32> = vec3<f32>(0.3, 0.3, 0.3);
        for (var i = 0u; i < 3; i += 1u) {
            let light = u_lights[i];
            let shadow = fetch_shadow(i, light.view_proj * in.world_position);

            light_color += phong(light, normal, in, shadow);
        }

        return vec4<f32>(light_color, 1.0) * material_color;
}