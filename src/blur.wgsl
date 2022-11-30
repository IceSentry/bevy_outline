@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var stencil_sampler: sampler;

// TODO should probably precompute the texelsize
// TODO use gaussian blur instead of box blur

// sigma = 10
// generated kernel coefficients
// taken from https://lisyarus.github.io/blog/graphics/2022/04/21/compute-blur.html
// var<private> GAUSSIAN_COEFFICIENTS: array<f32, 33> = array<f32, 33>(
//     0.012318109844189502,
//     0.014381474814203989,
//     0.016623532195728208,
//     0.019024086115486723,
//     0.02155484948872149,
//     0.02417948052890078,
//     0.02685404941667096,
//     0.0295279624870386,
//     0.03214534135442581,
//     0.03464682117793548,
//     0.0369716985390341,
//     0.039060328279673276,
//     0.040856643282313365,
//     0.04231065439216247,
//     0.043380781642569775,
//     0.044035873841196206,
//     0.04425662519949865,
//     0.044035873841196206,
//     0.043380781642569775,
//     0.04231065439216247,
//     0.040856643282313365,
//     0.039060328279673276,
//     0.0369716985390341,
//     0.03464682117793548,
//     0.03214534135442581,
//     0.0295279624870386,
//     0.02685404941667096,
//     0.02417948052890078,
//     0.02155484948872149,
//     0.019024086115486723,
//     0.016623532195728208,
//     0.014381474814203989,
//     0.012318109844189502
// );

let KERNEL_SIZE: f32 = 10.0;

@fragment
fn vertical_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = 1.0 / vec2<f32>(textureDimensions(input_texture));

    // Vertical Gaussian blur
    // let direction = vec2(0.0, dims.y);
    // var sum = vec4(0.0);
    // for (var i = 0; i < 33; i += 1) {
    //     let tc = uv + direction * f32(i - 16);
    //     sum += GAUSSIAN_COEFFICIENTS[i] * textureSample(input_texture, stencil_sampler, tc);
    // }
    // return sum;

    // Vertical box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * KERNEL_SIZE + 1.0;
    for (var y = 0.0; y < samples; y += 1.0) {
        let offset = vec2(0.0, y - KERNEL_SIZE);
        sum += textureSample(input_texture, stencil_sampler, uv + offset * dims);
    }
    return sum / samples;
}

@fragment
fn horizontal_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let dims = 1.0 / vec2<f32>(textureDimensions(input_texture));

    // Horizontal Gaussian blur
    // let direction = vec2(dims.x, 0.0);
    // var sum = vec4(0.0);
    // for (var i = 0; i < 33; i += 1) {
    //     let tc = uv + direction * f32(i - 16);
    //     sum += GAUSSIAN_COEFFICIENTS[i] * textureSample(input_texture, stencil_sampler, tc);
    // }
    // return sum;

    // Horizontal box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * KERNEL_SIZE + 1.0;
    for (var x = 0.0; x < samples; x += 1.0) {
        let offset = vec2(x - KERNEL_SIZE, 0.0);
        sum += textureSample(input_texture, stencil_sampler, uv + offset * dims);
    }
    return sum / samples;
}
