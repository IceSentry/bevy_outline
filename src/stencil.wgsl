#ifndef MAX_CASCADES_PER_LIGHT
    #define MAX_CASCADES_PER_LIGHT 1
#endif

#ifndef MAX_DIRECTIONAL_LIGHTS
    #define MAX_DIRECTIONAL_LIGHTS 1
#endif

#import bevy_pbr::mesh_view_types
#import bevy_pbr::mesh_types

@group(0) @binding(0)
var<uniform> view: View;

struct StencilUniform {
    color: vec4<f32>,
};

@group(1) @binding(0)
var<uniform> mesh: Mesh;
#ifdef SKINNED
@group(1) @binding(1)
var<uniform> joint_matrices: SkinnedMesh;
#import bevy_pbr::skinning
#endif

// NOTE: Bindings must come before functions that use them!
#import bevy_pbr::mesh_functions

@group(2) @binding(0)
var<uniform> stencil_uniform: StencilUniform;

struct Vertex {
    @location(0) position: vec3<f32>,
#ifdef SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
#ifdef SKINNED
    let model = skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    let model = mesh.model;
#endif
    var out: VertexOutput;
    out.clip_position = view.view_proj * model * vec4<f32>(vertex.position, 1.0);
    return out;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return stencil_uniform.color;
}
