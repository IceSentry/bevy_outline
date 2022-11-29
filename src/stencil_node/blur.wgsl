@group(0) @binding(0)
var stencil: texture_2d<f32>;
@group(0) @binding(1)
var stencil_sampler: sampler;

let KERNEL_SIZE: f32 = 3.0;

// TODO should probably precompute the texelsize

@fragment
fn vertical_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = 1.0 / vec2<f32>(textureDimensions(stencil));

    // Vertical box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * KERNEL_SIZE + 1.0;
    for (var y = 0.0; y < samples; y += 1.0) {
        let offset = vec2(0.0, y - KERNEL_SIZE);
        sum += textureSample(stencil, stencil_sampler, uv + offset * dims);
    }

    return sum / samples;
}

@fragment
fn horizontal_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = 1.0 / vec2<f32>(textureDimensions(stencil));

    // Vertical box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * KERNEL_SIZE + 1.0;
    for (var x = 0.0; x < samples; x += 1.0) {
        let offset = vec2(x - KERNEL_SIZE, 0.0);
        sum += textureSample(stencil, stencil_sampler, uv + offset * dims);
    }

    return sum / samples;
}