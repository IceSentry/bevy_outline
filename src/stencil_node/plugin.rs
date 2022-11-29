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
            BindGroupDescriptor, BindGroupEntry, BindingResource, Extent3d, PipelineCache,
            SpecializedMeshPipelines, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, TextureCache},
        view::{ExtractedView, VisibleEntities},
        Extract, RenderApp, RenderStage,
    },
    utils::HashMap,
};

use crate::{
    plugin::Outline,
    stencil_node::{OutlinePipelines, BLUR_SHADER_HANDLE, COMBINE_SHADER_HANDLE},
};

use super::{
    DrawMeshStencil, MeshStencil, OutlineBindGroups, StencilPipeline, StencilTexture,
    STENCIL_SHADER_HANDLE,
};

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

        load_internal_asset!(app, BLUR_SHADER_HANDLE, "blur.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            COMBINE_SHADER_HANDLE,
            "combine.wgsl",
            Shader::from_wgsl
        );

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<OutlinePipelines>()
            .init_resource::<DrawFunctions<MeshStencil>>()
            .add_render_command::<MeshStencil, SetItemPipeline>()
            .add_render_command::<MeshStencil, DrawMeshStencil>()
            .init_resource::<StencilPipeline>()
            .init_resource::<SpecializedMeshPipelines<StencilPipeline>>()
            .add_system_to_stage(RenderStage::PhaseSort, sort_phase_system::<MeshStencil>)
            .add_system_to_stage(RenderStage::Prepare, prepare_stencil_textures)
            .add_system_to_stage(RenderStage::Extract, extract_stencil_phase)
            .add_system_to_stage(RenderStage::Queue, queue_mesh_stencil)
            .add_system_to_stage(RenderStage::Queue, queue_outline_bind_groups);
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

// Bind the required data for the outline bind groups
fn queue_outline_bind_groups(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipelines: Res<OutlinePipelines>,
    views: Query<(Entity, &StencilTexture)>,
) {
    for (entity, textures) in &views {
        let blur_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_blur_bind_group"),
            layout: &pipelines.blur_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&textures.stencil_texture.default_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&pipelines.sampler),
                },
            ],
        });

        let combine_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_combine_bind_group"),
            layout: &pipelines.combine_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Sampler(&pipelines.sampler),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&textures.stencil_texture.default_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(
                        &textures.vertical_blur_texture.default_view,
                    ),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(
                        &textures.horizontal_blur_texture.default_view,
                    ),
                },
            ],
        });

        commands.entity(entity).insert(OutlineBindGroups {
            blur_bind_group,
            combine_bind_group,
        });
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
    let mut vertical_blur_textures = HashMap::default();
    let mut horizontal_blur_textures = HashMap::default();

    for (entity, camera) in &views {
        let Some(UVec2 { x, y }) = camera.physical_viewport_size else {
            continue;
        };
        let size = Extent3d {
            width: x,
            height: y,
            depth_or_array_layers: 1,
        };

        let stencil_desc = TextureDescriptor {
            label: Some("stencil_output"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::bevy_default(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        };
        let stencil_texture = stencil_textures
            .entry(camera.target.clone())
            .or_insert_with(|| texture_cache.get(&render_device, stencil_desc.clone()))
            .clone();

        let blur_desc = TextureDescriptor {
            label: Some("blur_output"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::bevy_default(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        };
        let vertical_blur_texture = vertical_blur_textures
            .entry(camera.target.clone())
            .or_insert_with(|| texture_cache.get(&render_device, blur_desc.clone()))
            .clone();

        let horizontal_blur_texture = horizontal_blur_textures
            .entry(camera.target.clone())
            .or_insert_with(|| texture_cache.get(&render_device, blur_desc.clone()))
            .clone();

        commands.entity(entity).insert(StencilTexture {
            stencil_texture,
            vertical_blur_texture,
            horizontal_blur_texture,
        });
    }
}
