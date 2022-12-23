@group(0) @binding(0)
var stencil_sampler: sampler;
@group(0) @binding(1)
var stencil: texture_2d<f32>;
@group(0) @binding(2)
var blur_texture: texture_2d<f32>;

// TODO solid outlines

@fragment
fn combine(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let stencil_color = textureSample(stencil, stencil_sampler, uv);
    let blur_color = textureSample(blur_texture, stencil_sampler, uv);

    if blur_color.a > 0.0 && stencil_color.a == 0.0 {
        return vec4(1.0);
    }
    return mix(blur_color, vec4(0.0), stencil_color.a);
}