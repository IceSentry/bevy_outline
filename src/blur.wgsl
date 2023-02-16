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

fn sample_stencil(uv: vec2<f32>, offset: vec2<f32>) -> vec4<f32> {
    return textureSample(input_texture, stencil_sampler, uv + offset * settings.dims);
}

// TODO this should be done in a separate pass to avoid doing a bunch of work
fn max_filter(uv: vec2<f32>) -> vec4<f32>{
    var col = vec4(0.0);
    let size = i32(settings.size);
    for (var x = -size; x <= size; x++) {
        for (var y = -size; y <= size; y++) {
            let offset = vec2(f32(x), f32(y));
            col = max(col, sample_stencil(uv, offset));
        }
    }
    return col;
}

var<private> OFFSETS: array<f32, 3> = array<f32, 3>(
    0.0,
    1.3846153846,
    3.2307692308
);
var<private> GAUSSIAN_WEIGHTS: array<f32, 3> = array<f32, 3>(
    0.2270270270,
    0.3162162162,
    0.0702702703
);

// Technique from https://www.rastergrid.com/blog/2010/09/efficient-gaussian-blur-with-linear-sampling/
fn gaussian_blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    var sum = sample_stencil(uv, vec2(0.0)) * GAUSSIAN_WEIGHTS[0];
    for (var i = 1; i < 3; i++) {
        sum += sample_stencil(uv , vec2(OFFSETS[i]) * settings.size * direction) * GAUSSIAN_WEIGHTS[i];
        sum += sample_stencil(uv , -vec2(OFFSETS[i]) * settings.size * direction) * GAUSSIAN_WEIGHTS[i];
    }
    return sum;
}

fn box_blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    var sum = vec4(0.0);
    let samples = 2.0 * settings.size + 1.0;
    for (var i = 0.0; i < samples; i += 1.0) {
        let offset = vec2(i - settings.size) * direction;
        sum += sample_stencil(uv, offset);
    }
    return sum / samples;
}

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let sample_uv = settings.viewport.xy + uv * settings.viewport.zw;

    #ifdef HORIZONTAL
    let direction = vec2(1.0, 0.0);
    #else // HORIZONTAL
    let direction = vec2(0.0, 1.0);
    #endif // HORIZONTAL

    #ifdef GAUSSIAN_BLUR
    return gaussian_blur(sample_uv, direction);
    #else // GAUSSIAN_BLUR
    return box_blur(sample_uv, direction);
    #endif // GAUSSIAN_BLUR
}

