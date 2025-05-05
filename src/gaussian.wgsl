@group(0) @binding(0)
var input_texture: texture_2d_array<f32>;

@group(0) @binding(1)
var output_texture: texture_storage_2d_array<rgba32float, write>;

@group(0) @binding(2)
var<uniform> is_vertical: u32;

const KERNEL_RADIUS: i32 = 5;
const KERNEL_SIZE: i32 = (KERNEL_RADIUS * 2 + 1);
const INV_KERNEL_SIZE: f32 = 1.0 / f32(KERNEL_SIZE);

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    let width = i32(dims.x);
    let height = i32(dims.y);
    let layer = i32(global_id.z);
    let x = i32(global_id.x);
    let y = i32(global_id.y);

    if (x >= width || y >= height) {
        return;
    }

    var sum = vec4<f32>(0.0);
    if (is_vertical == 0u) {
        // Horizontal box blur
        for (var i = -KERNEL_RADIUS; i <= KERNEL_RADIUS; i++) {
            let sx = clamp(x + i, 0, width - 1);
            let sample = textureLoad(input_texture, vec2<i32>(sx, y), layer, 0);
            sum += sample;
        }
    } else {
        // Vertical box blur
        for (var i = -KERNEL_RADIUS; i <= KERNEL_RADIUS; i++) {
            let sy = clamp(y + i, 0, height - 1);
            let sample = textureLoad(input_texture, vec2<i32>(x, sy), layer, 0);
            sum += sample;
        }
    }

    let blurred = sum * INV_KERNEL_SIZE;
    textureStore(output_texture, vec2<i32>(x, y), layer, blurred);
}
