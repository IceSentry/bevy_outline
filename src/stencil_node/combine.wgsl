
@group(0) @binding(0)
var stencil_sampler: sampler;
@group(0) @binding(1)
var stencil: texture_2d<f32>;
@group(0) @binding(2)
var blur_texture: texture_2d<f32>;

// TODO mix with target color instead of discarding on black

@fragment
fn combine(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let color = textureSample(stencil, stencil_sampler, uv);
    let blur_color = textureSample(blur_texture, stencil_sampler, uv);
    var out_color = vec4(0.0);
    if all(color == vec4(0.0, 0.0, 0.0, 1.0)) {
        out_color = blur_color;
        if all(out_color.xyz == vec3(0.0)) {
            discard;
        }
        return out_color;
    } else {
        discard;
    }
}