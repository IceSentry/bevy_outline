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

var<private> OFFSETS_5: array<f32, 1> = array<f32, 1>(
    1.3333333333333333,
);
var<private> GAUSSIAN_WEIGHTS_5: array<f32, 3> = array<f32, 3>(
    0.29411764705882354,
    0.35294117647058826,
    0.35294117647058826
);

fn blur5(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    let offset = vec2(1.3333333333333333) * direction;
    var color = vec4(0.0);
    color += sample_stencil(uv, vec2(0.0)) * 0.29411764705882354;
    color += sample_stencil(uv, offset) * 0.35294117647058826;
    color += sample_stencil(uv, -offset) * 0.35294117647058826;
    return color;
}

fn blur9(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    let offset1 = vec2(1.3846153846) * direction;
    let offset2 = vec2(3.2307692308) * direction;
    var color = vec4(0.0);
    color += sample_stencil(uv, vec2(0.0)) * 0.2270270270;
    color += sample_stencil(uv, offset1) *  0.3162162162;
    color += sample_stencil(uv, -offset1) *  0.3162162162;
    color += sample_stencil(uv, offset2) * 0.0702702703;
    color += sample_stencil(uv, -offset2) * 0.0702702703;
    return color;
}

fn blur13(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    let offset1 = vec2(1.411764705882353) * direction;
    let offset2 = vec2(3.2941176470588234) * direction;
    let offset3 = vec2(5.176470588235294) * direction;

    var color = vec4(0.0);
    color += sample_stencil(uv, vec2(0.0)) * 0.1964825501511404;
    color += sample_stencil(uv, offset1) *  0.2969069646728344;
    color += sample_stencil(uv, -offset1) *  0.2969069646728344;
    color += sample_stencil(uv, offset2) * 0.09447039785044732;
    color += sample_stencil(uv, -offset2) * 0.09447039785044732;
    color += sample_stencil(uv, offset3) * 0.010381362401148057;
    color += sample_stencil(uv, -offset3) * 0.010381362401148057;
    return color;
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

// based on https://www.rastergrid.com/blog/2010/09/efficient-gaussian-blur-with-linear-sampling/
fn gaussian_blur(uv: vec2<f32>, direction: vec2<f32>) -> vec4<f32>{
    var sum = sample_stencil(uv, vec2(0.0)) * GAUSSIAN_WEIGHTS[0];
    let size = settings.size;
    for (var i = 1; i < 3; i++) {
        let offset  = vec2(OFFSETS[i]) * size * direction;
        sum += sample_stencil(uv , offset) * GAUSSIAN_WEIGHTS[i];
        sum += sample_stencil(uv , -offset) * GAUSSIAN_WEIGHTS[i];
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
    // return blur13(sample_uv, direction * settings.size);
    #else // GAUSSIAN_BLUR
    return box_blur(sample_uv, direction);
    #endif // GAUSSIAN_BLUR
}

