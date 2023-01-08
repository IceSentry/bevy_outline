@group(0) @binding(0)
var stencil_sampler: sampler;
@group(0) @binding(1)
var stencil: texture_2d<f32>;
@group(0) @binding(2)
var blur_texture: texture_2d<f32>;
@group(0) @binding(3)
var<uniform> intensity: f32;

// TODO solid outlines

@fragment
fn combine(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let stencil_color = textureSample(stencil, stencil_sampler, uv);
    var blur_color = textureSample(blur_texture, stencil_sampler, uv);

    // don't render outlines if they overlap
    if any(stencil_color.xyz > vec3(0.0)) {
        return vec4(0.0);
    }

    let outline = max(vec4(0.0), blur_color - stencil_color);
    return outline * intensity;
}