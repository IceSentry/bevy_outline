use bevy::{
    asset::load_internal_asset,
    pbr::{MeshPipelineKey, MeshUniform},
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_asset::RenderAssets,
        render_phase::{
            sort_phase_system, AddRenderCommand, DrawFunctions, RenderPhase, SetItemPipeline,
        },
        render_resource::{
            Extent3d, PipelineCache, SpecializedMeshPipelines, TextureDescriptor, TextureDimension,
            TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, TextureCache},
        view::{ExtractedView, VisibleEntities},
        Extract, RenderApp, RenderStage,
    },
    utils::HashMap,
};

use crate::plugin::Outline;

use super::{DrawMeshStencil, MeshStencil, StencilPipeline, StencilTexture, STENCIL_SHADER_HANDLE};

/// This plugins sets up all the required systems and resources for the stencil phase
pub struct StencilPassPlugin;
impl Plugin for StencilPassPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            STENCIL_SHADER_HANDLE,
            "stencil.wgsl",
            Shader::from_wgsl
        );

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<DrawFunctions<MeshStencil>>()
            .add_render_command::<MeshStencil, SetItemPipeline>()
            .add_render_command::<MeshStencil, DrawMeshStencil>()
            .init_resource::<StencilPipeline>()
            .init_resource::<SpecializedMeshPipelines<StencilPipeline>>()
            .add_system_to_stage(RenderStage::PhaseSort, sort_phase_system::<MeshStencil>)
            .add_system_to_stage(RenderStage::Prepare, prepare_stencil_textures)
            .add_system_to_stage(RenderStage::Extract, extract_stencil_phase)
            .add_system_to_stage(RenderStage::Queue, queue_mesh_stencil);
    }
}

/// Make sure all 3d cameras have a [`MeshStencil`] [`RenderPhase`]
fn extract_stencil_phase(mut commands: Commands, cameras: Extract<Query<Entity, With<Camera3d>>>) {
    for entity in cameras.iter() {
        commands
            .get_or_spawn(entity)
            .insert(RenderPhase::<MeshStencil>::default());
    }
}

/// Add any visible entity with a mesh and an [`Outline`] to the stencil_phase
fn queue_mesh_stencil(
    stencil_draw_functions: Res<DrawFunctions<MeshStencil>>,
    stencil_pipeline: Res<StencilPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<StencilPipeline>>,
    mut pipeline_cache: ResMut<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    outline_meshes: Query<(Entity, &Handle<Mesh>, &MeshUniform), With<Outline>>,
    mut views: Query<(
        &ExtractedView,
        &mut VisibleEntities,
        &mut RenderPhase<MeshStencil>,
    )>,
) {
    let draw_outline = stencil_draw_functions
        .read()
        .get_id::<DrawMeshStencil>()
        .unwrap();

    for (view, visible_entities, mut stencil_phase) in views.iter_mut() {
        let view_matrix = view.transform.compute_matrix();
        let inv_view_row_2 = view_matrix.inverse().row(2);

        for visible_entity in visible_entities.entities.iter().copied() {
            let Ok((entity, mesh_handle, mesh_uniform)) = outline_meshes.get(visible_entity) else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_handle) else {
                continue;
            };

            let key = MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);

            let pipeline = pipelines
                .specialize(&mut pipeline_cache, &stencil_pipeline, key, &mesh.layout)
                .unwrap();

            stencil_phase.add(MeshStencil {
                entity,
                pipeline,
                draw_function: draw_outline,
                distance: inv_view_row_2.dot(mesh_uniform.transform.col(3)),
            });
        }
    }
}

// Prepares the textures used to render the stencil for each camera
fn prepare_stencil_textures(
    mut commands: Commands,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    views: Query<(Entity, &ExtractedCamera)>,
) {
    let mut stencil_textures = HashMap::default();
    for (entity, camera) in &views {
        let Some(UVec2 { x, y }) = camera.physical_viewport_size else {
            continue;
        };
        let stencil_desc = TextureDescriptor {
            label: Some("stencil_output"),
            size: Extent3d {
                // Scale down the view to make the blur pass faster
                // It doesn't need to be super precise anyway since it's gonna be blurred
                // TODO Consider making it configurable
                width: (x / 2).max(1),
                height: (y / 2).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::bevy_default(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        };
        let texture = stencil_textures
            .entry(camera.target.clone())
            .or_insert_with(|| texture_cache.get(&render_device, stencil_desc.clone()))
            .clone();

        commands.entity(entity).insert(StencilTexture { texture });
    }
}
