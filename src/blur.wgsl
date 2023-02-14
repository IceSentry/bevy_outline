struct BlurSettings {
    size: f32,
    dims: vec2<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var stencil_sampler: sampler;
@group(0) @binding(2)
var<uniform> settings: BlurSettings;

// TODO use gaussian blur instead of box blur

fn get_sample_uv(uv: vec2<f32>) -> vec2<f32> {
    return settings.viewport.xy + uv * settings.viewport.zw;
}

fn sample_stencil(uv: vec2<f32>, offset: vec2<f32>) -> vec4<f32> {
    return textureSample(input_texture, stencil_sampler, uv + offset * settings.dims);
}

@fragment
fn vertical_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let sample_uv = get_sample_uv(uv);

    // Vertical box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * settings.size + 1.0;
    for (var y = 0.0; y < samples; y += 1.0) {
        let offset = vec2(0.0, y - settings.size);
        sum += sample_stencil(sample_uv, offset);
    }
    return sum / samples;
}

@fragment
fn horizontal_blur(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let sample_uv = get_sample_uv(uv);

    // Horizontal box blur
    var sum = vec4(0.0, 0.0, 0.0, 0.0);
    let samples = 2.0 * settings.size + 1.0;
    for (var x = 0.0; x < samples; x += 1.0) {
        let offset = vec2(x - settings.size, 0.0);
        sum += sample_stencil(sample_uv, offset);
    }
    return sum / samples;
}

