struct MaxFilterSettings {
    size: f32,
    dims: vec2<f32>,
    viewport: vec4<f32>,
};

@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var stencil_sampler: sampler;
@group(0) @binding(2)
var<uniform> settings: MaxFilterSettings;

fn get_sample_uv(uv: vec2<f32>) -> vec2<f32> {
    return settings.viewport.xy + uv * settings.viewport.zw;
}

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let sample_uv = get_sample_uv(uv);
    var col = vec4(0.0);
    let size = i32(settings.size);
    for (var x = -size; x <= size; x++) {
        for (var y = -size; y <= size; y++) {
            let offset = vec2(f32(x), f32(y)) * settings.dims;
            col = max(col, textureSample(input_texture, stencil_sampler, sample_uv + offset));
        }
    }
    return col;
    // return vec4(1.0);
}
