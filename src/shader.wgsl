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
    var out = VertexOutput();
    out.out_position = camera.view_proj * world_position;
    out.tex_coords = in.tex_coords;
    out.world_position = world_position;
    out.world_normal = normalize(mat3x3<f32>(world[0].xyz, world[1].xyz, world[2].xyz) * in.normal);
    return out;
}

struct Light {
    position: vec4<f32>,
    color: vec4<f32>,
    model: mat4x4<f32>,
    view_proj: mat4x4<f32>,
}
@group(3) @binding(0)
var<storage, read> s_lights: array<Light>;
@group(3) @binding(0)
var<uniform> u_lights: array<Light, 10>;
@group(3) @binding(1) var t_shadow: texture_2d_array<f32>;
@group(3) @binding(2) var sampler_shadow: sampler;

fn fetch_shadow(light_id: u32, ls_pos: vec4<f32>) -> f32 {
    if (ls_pos.w <= 0.0) {
        return 1.0;
    }
    // compensate for the Y-flip difference between the NDC and texture coordinates
    let flip_correction = vec2<f32>(0.5, -0.5);
    // compute texture coordinates for shadow lookup
    let proj_correction = 1.0 / ls_pos.w;
    let light_local = ls_pos.xy * flip_correction * proj_correction + vec2<f32>(0.5, 0.5);
    let depth = ls_pos.z * proj_correction;

    let moments = textureSampleLevel(t_shadow, sampler_shadow, light_local, i32(light_id), 0.0);
    let reversed_moments = convert_optimized_moments(moments);

    return compute_msm_shadow_intensity(reversed_moments, depth);
}

// Reverts the projection of the moments done in the shadow pass
fn convert_optimized_moments(optimized: vec4<f32>) -> vec4<f32> {
    var adjusted = optimized;
    adjusted.x -= 0.035955884801;

    let M_inv = mat4x4<f32>(
        vec4<f32>(0.2227744146,  0.1549679261,  0.1451988946,  0.163127443),
        vec4<f32>(0.0771972861,  0.1394629426,  0.2120202157,  0.2591432266),
        vec4<f32>(0.7926986636,  0.7963415838,  0.7258694464,  0.6539092497),
        vec4<f32>(0.0319417555, -0.1722823173, -0.2758014811, -0.3376131734)
    );

    return M_inv * adjusted;
}


fn compute_msm_shadow_intensity(moments: vec4<f32>, fragmentDepth: f32) -> f32 {
    // TODO: bias
    let b = moments + 0.005;

    // cholesky
    let l32_d22 = -b.x * b.y + b.z;
    let d22 = -b.x * b.x + b.y;
    let squaredDepthVar = -b.y * b.y + b.w;

    let d33_d22 = dot(
        vec2<f32>(squaredDepthVar, -l32_d22),
        vec2<f32>(d22,          l32_d22)
    );

    let inv_d22 = 1.0 / d22;
    let l32 = l32_d22 * inv_d22;

    // build quadratic equation to find z1 and z2
    let z0 = fragmentDepth;
    var c = vec3<f32>(1.0, z0 - b.x, z0 * z0);

    c.z -= b.y + l32 * c.y;
    c.y *= inv_d22;
    c.z *= d22 / d33_d22;
    c.y -= l32 * c.z;
    c.x -= dot(c.yz, b.xy);

    let inv_c2 = 1.0 / c.z;
    let p = c.y * inv_c2;
    let q = c.x * inv_c2;
    let radicand = (p * p * 0.25) - q;
    let r = sqrt(max(radicand, 0.0));
    let z1 = -0.5 * p - r;
    let z2 = -0.5 * p + r;

    var switchVals = vec4<f32>(0.0);
    // z2 < z0  → [ z1, z0, 1, 1 ]
    if (z2 < z0) {
        switchVals = vec4<f32>(z1, z0, 1.0, 1.0);
    } else if (z1 < z0) {
        // z1 < z0  → [ z0, z1, 0, 1 ]
        switchVals = vec4<f32>(z0, z1, 0.0, 1.0);
    }

    let numerator = switchVals.x * z2 - b.x * (switchVals.x + z2 + b.y);
    let denominator = (z2 - switchVals.y) * (z0 - z1);
    let quotient = numerator / denominator;

    let rawLight = switchVals.y + switchVals.z * quotient;
    let intensity = clamp(rawLight, 0.0, 1.0);

    return 1.0 - intensity;
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

fn phong (light: Light, normal: vec3<f32>, in: VertexOutput) -> vec3<f32> {
    let light_world_position = light.model * light.position;
    let light_dir = normalize(light_world_position.xyz - in.world_position.xyz);

    let diffuse = max(0.0, dot(normal, light_dir));

    let view_dir = normalize(camera.position.xyz - in.world_position.xyz);
    let reflect_dir  = reflect(-light_dir, in.world_normal);
    let specular = pow(max(0.0, dot(normal, reflect_dir)), (10 * material.shininess));

    return diffuse * light.color.xyz + specular * material.specular.xyz;
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
        let light_proj = light.view_proj * in.world_position;
        let shadow = fetch_shadow(i, light_proj);

        light_color += phong(light, normal, in) * shadow;
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
            let light_proj = light.view_proj * in.world_position;
            let shadow = fetch_shadow(i, light_proj);

            light_color += phong(light, normal, in) * shadow;
        }

        return vec4<f32>(light_color, 1.0) * material_color;
}