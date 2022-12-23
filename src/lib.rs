mod fullscreen_vertex_shader;
pub mod node;
mod utils;

use bevy::{
    asset::load_internal_asset,
    core_pipeline::core_3d,
    pbr::{
        DrawMesh, MeshPipeline, MeshPipelineKey, MeshUniform, SetMeshBindGroup,
        SetMeshViewBindGroup,
    },
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::InnerMeshVertexBufferLayout,
        render_asset::RenderAssets,
        render_graph::RenderGraph,
        render_phase::{
            sort_phase_system, AddRenderCommand, CachedRenderPipelinePhaseItem, DrawFunctionId,
            DrawFunctions, EntityPhaseItem, PhaseItem, RenderPhase, SetItemPipeline,
        },
        render_resource::{
            AddressMode, BindGroup, BindGroupDescriptor, BindGroupLayout,
            BindGroupLayoutDescriptor, BindingResource, BindingType, BlendState,
            CachedRenderPipelineId, Extent3d, FilterMode, PipelineCache, RenderPipelineDescriptor,
            Sampler, SamplerBindingType, SamplerDescriptor, SpecializedMeshPipeline,
            SpecializedMeshPipelineError, SpecializedMeshPipelines, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
            TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, CachedTexture, TextureCache},
        view::{ExtractedView, VisibleEntities},
        Extract, RenderApp, RenderStage,
    },
    utils::{FixedState, FloatOrd, Hashed},
};
use utils::{color_target, fragment_state, RenderPipelineDescriptorBuilder};

use crate::{fullscreen_vertex_shader::FULLSCREEN_SHADER_HANDLE, node::OutlineNode};

const STENCIL_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 15139276207022888006);

const BLUR_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 14687827633551304793);

const COMBINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 13593741836324854485);

#[derive(Component, Clone, Copy)]
pub struct Outline;

impl ExtractComponent for Outline {
    type Query = &'static Self;

    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<Self::Query>) -> Self {
        *item
    }
}

pub mod graph {
    pub mod input {
        pub const VIEW_ENTITY: &str = "view_entity";
    }

    pub mod node {
        pub const OUTLINE_PASS: &str = "outline_pass";
    }
}

pub struct BlurredOutlinePlugin;
impl Plugin for BlurredOutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            FULLSCREEN_SHADER_HANDLE,
            "fullscreen_vertex_shader/fullscreen.wgsl",
            Shader::from_wgsl
        );
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

        app.add_plugin(ExtractComponentPlugin::<Outline>::default());

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<OutlinePipelines>()
            .init_resource::<StencilPipeline>()
            .init_resource::<SpecializedMeshPipelines<StencilPipeline>>()
            .init_resource::<DrawFunctions<MeshStencil>>()
            .add_render_command::<MeshStencil, DrawMeshStencil>()
            .add_system_to_stage(RenderStage::PhaseSort, sort_phase_system::<MeshStencil>)
            .add_system_to_stage(RenderStage::Extract, extract_stencil_phase)
            .add_system_to_stage(RenderStage::Prepare, prepare_outline_resources)
            .add_system_to_stage(RenderStage::Queue, queue_mesh_stencil);

        {
            let stencil_node = OutlineNode::new(&mut render_app.world);
            let mut graph = render_app.world.resource_mut::<RenderGraph>();
            let draw_3d_graph = graph.get_sub_graph_mut(core_3d::graph::NAME).unwrap();

            draw_3d_graph.add_node(graph::node::OUTLINE_PASS, stencil_node);

            draw_3d_graph
                .add_slot_edge(
                    draw_3d_graph.input_node().unwrap().id,
                    graph::input::VIEW_ENTITY,
                    graph::node::OUTLINE_PASS,
                    OutlineNode::IN_VIEW,
                )
                .unwrap();

            // MAIN_PASS -> OUTLINE
            draw_3d_graph
                .add_node_edge(core_3d::graph::node::MAIN_PASS, graph::node::OUTLINE_PASS)
                .unwrap();
        }
    }
}

struct MeshStencil {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
}

impl PhaseItem for MeshStencil {
    type SortKey = FloatOrd;

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }
}

impl EntityPhaseItem for MeshStencil {
    fn entity(&self) -> Entity {
        self.entity
    }
}

impl CachedRenderPipelinePhaseItem for MeshStencil {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

type DrawMeshStencil = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawMesh,
);

pub struct StencilPipeline {
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for StencilPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        StencilPipeline { mesh_pipeline }
    }
}

impl SpecializedMeshPipeline for StencilPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &Hashed<InnerMeshVertexBufferLayout, FixedState>,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut desc = self.mesh_pipeline.specialize(key, layout)?;

        desc.label = Some("mesh_stencil_pipeline".into());

        desc.layout = Some(vec![
            self.mesh_pipeline.view_layout.clone(),
            self.mesh_pipeline.mesh_layout.clone(),
            // TODO add bind group with configurable color
        ]);
        desc.vertex.shader = STENCIL_SHADER_HANDLE.typed::<Shader>();
        desc.fragment = fragment_state(STENCIL_SHADER_HANDLE, "fragment", &[color_target(None)]);
        desc.depth_stencil = None;

        Ok(desc)
    }
}

#[derive(Component)]
struct OutlineResources {
    stencil_texture: CachedTexture,
    vertical_blur_texture: CachedTexture,
    horizontal_blur_texture: CachedTexture,
    vertical_blur_bind_group: BindGroup,
    horizontal_blur_bind_group: BindGroup,
    combine_bind_group: BindGroup,
}

struct OutlinePipelines {
    sampler: Sampler,
    blur_bind_group_layout: BindGroupLayout,
    combine_bind_group_layout: BindGroupLayout,
    horizontal_blur_pipeline: CachedRenderPipelineId,
    vertical_blur_pipeline: CachedRenderPipelineId,
    combine_pipeline: CachedRenderPipelineId,
}

impl FromWorld for OutlinePipelines {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..default()
        });
        let texture = BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        };

        let blur_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("blur_bind_group_layout"),
                entries: &bind_group_layout_entries![
                    // stencil texture
                    0 => texture,
                    // sampler
                    1 => BindingType::Sampler(SamplerBindingType::Filtering),
                ],
            });
        let combine_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("combine_bind_group_layout"),
                entries: &bind_group_layout_entries![
                    // sampler
                    0 => BindingType::Sampler(SamplerBindingType::Filtering),
                    // stencil texture
                    1 => texture,
                    // blur texture
                    2 => texture,
                ],
            });

        let mut pipeline_cache = world.resource_mut::<PipelineCache>();

        let vertical_blur_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::default_fullscreen()
                .label("vertical_blur_pipeline")
                .fragment(BLUR_SHADER_HANDLE, "vertical_blur", &[color_target(None)])
                .layout(vec![blur_bind_group_layout.clone()])
                .build(),
        );

        let horizontal_blur_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::default_fullscreen()
                .label("horizontal_blur_pipeline")
                .fragment(BLUR_SHADER_HANDLE, "horizontal_blur", &[color_target(None)])
                .layout(vec![blur_bind_group_layout.clone()])
                .build(),
        );

        let combine_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::default_fullscreen()
                .label("combine_pipeline")
                .fragment(
                    COMBINE_SHADER_HANDLE,
                    "combine",
                    &[color_target(Some(BlendState::ALPHA_BLENDING))],
                )
                .layout(vec![combine_bind_group_layout.clone()])
                .build(),
        );

        Self {
            sampler,
            blur_bind_group_layout,
            combine_bind_group_layout,
            vertical_blur_pipeline,
            horizontal_blur_pipeline,
            combine_pipeline,
        }
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

/// Prepares the textures and the bind groups used to render the outline
fn prepare_outline_resources(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipelines: Res<OutlinePipelines>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &ExtractedCamera)>,
) {
    for (entity, camera) in &cameras {
        let Some(UVec2 { x, y }) = camera.physical_viewport_size else {
            continue;
        };

        // TODO make this configurable
        let size = Extent3d {
            width: (x / 1).max(1),
            height: (y / 1).max(1),
            depth_or_array_layers: 1,
        };

        let base_desc = TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::bevy_default(),
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        };

        let stencil_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("stencil_output"),
                ..base_desc
            },
        );

        let blur_desc = TextureDescriptor {
            label: Some("blur_output"),
            ..base_desc
        };
        let vertical_blur_texture = texture_cache.get(&render_device, blur_desc.clone());
        let horizontal_blur_texture = texture_cache.get(&render_device, blur_desc.clone());

        let vertical_blur_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_vertical_blur_bind_group"),
            layout: &pipelines.blur_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::TextureView(&stencil_texture.default_view),
                1 => BindingResource::Sampler(&pipelines.sampler),
            ],
        });

        let horizontal_blur_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_horizontal_blur_bind_group"),
            layout: &pipelines.blur_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::TextureView(&vertical_blur_texture.default_view),
                1 => BindingResource::Sampler(&pipelines.sampler),
            ],
        });

        let combine_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_combine_bind_group"),
            layout: &pipelines.combine_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::Sampler(&pipelines.sampler),
                1 => BindingResource::TextureView(&stencil_texture.default_view),
                2 => BindingResource::TextureView(&horizontal_blur_texture.default_view),
            ],
        });

        commands.entity(entity).insert(OutlineResources {
            vertical_blur_bind_group,
            horizontal_blur_bind_group,
            combine_bind_group,
            stencil_texture,
            vertical_blur_texture,
            horizontal_blur_texture,
        });
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
