use bevy::{
    asset::load_internal_asset,
    ecs::{
        query::ROQueryItem,
        system::{
            lifetimeless::{Read, SRes},
            SystemParamItem,
        },
    },
    pbr::{
        DrawMesh, MeshPipeline, MeshPipelineKey, MeshUniform, SetMeshBindGroup,
        SetMeshViewBindGroup,
    },
    prelude::*,
    reflect::TypeUuid,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex, UniformComponentPlugin},
        mesh::InnerMeshVertexBufferLayout,
        render_asset::RenderAssets,
        render_phase::{
            sort_phase_system, AddRenderCommand, CachedRenderPipelinePhaseItem, DrawFunctionId,
            DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult, RenderPhase,
            SetItemPipeline, TrackedRenderPass,
        },
        render_resource::{
            BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor,
            BindingType, BufferBindingType, CachedRenderPipelineId, PipelineCache,
            RenderPipelineDescriptor, ShaderType, SpecializedMeshPipeline,
            SpecializedMeshPipelineError, SpecializedMeshPipelines,
        },
        renderer::RenderDevice,
        view::{ExtractedView, VisibleEntities},
        Extract, RenderApp, RenderSet,
    },
    utils::{FixedState, FloatOrd, Hashed},
};

use crate::{
    bind_group_entries, bind_group_layout_entries,
    utils::{color_target, fragment_state},
    Outline,
};

pub const STENCIL_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 15139276207022888006);

pub struct MeshStencilPlugin;
impl Plugin for MeshStencilPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            STENCIL_SHADER_HANDLE,
            "stencil.wgsl",
            Shader::from_wgsl
        );
        app.add_plugin(UniformComponentPlugin::<StencilUniform>::default());

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<StencilPipeline>()
            .init_resource::<SpecializedMeshPipelines<StencilPipeline>>()
            .init_resource::<DrawFunctions<MeshStencil>>()
            .add_render_command::<MeshStencil, DrawMeshStencil>()
            .add_system(sort_phase_system::<MeshStencil>.in_set(RenderSet::PhaseSort))
            .add_systems(
                (extract_stencil_phase, extract_stencil_uniform).in_schedule(ExtractSchedule),
            )
            .add_system(queue_stencil_bind_group.in_set(RenderSet::Queue))
            .add_system(queue_mesh_stencil.in_set(RenderSet::Queue));
    }
}

pub struct MeshStencil {
    pub distance: f32,
    pub pipeline: CachedRenderPipelineId,
    pub entity: Entity,
    pub draw_function: DrawFunctionId,
}

impl PhaseItem for MeshStencil {
    type SortKey = FloatOrd;

    fn entity(&self) -> Entity {
        self.entity
    }

    fn sort_key(&self) -> Self::SortKey {
        FloatOrd(self.distance)
    }

    fn draw_function(&self) -> DrawFunctionId {
        self.draw_function
    }
}

impl CachedRenderPipelinePhaseItem for MeshStencil {
    fn cached_pipeline(&self) -> CachedRenderPipelineId {
        self.pipeline
    }
}

#[derive(Component, ShaderType, Clone, Copy)]
pub struct StencilUniform {
    color: Color,
}

pub struct SetStencilBindGroup<const I: usize>;
impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetStencilBindGroup<I> {
    type Param = SRes<StencilBindGroup>;
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<DynamicUniformIndex<StencilUniform>>;

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        mesh_index: ROQueryItem<'w, Self::ItemWorldQuery>,
        resource: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_bind_group(I, &resource.into_inner().value, &[mesh_index.index()]);
        RenderCommandResult::Success
    }
}

pub type DrawMeshStencil = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetStencilBindGroup<2>,
    DrawMesh,
);

#[derive(Resource)]
pub struct StencilPipeline {
    mesh_pipeline: MeshPipeline,
    stencil_bind_group_layout: BindGroupLayout,
}

impl FromWorld for StencilPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let stencil_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("stencil_bind_group_layout"),
                entries: &bind_group_layout_entries![
                    0 => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(StencilUniform::min_size()),
                    },
                ],
            });

        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        StencilPipeline {
            mesh_pipeline,
            stencil_bind_group_layout,
        }
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

        let mut bind_group_layout = match key.msaa_samples() {
            1 => vec![self.mesh_pipeline.view_layout.clone()],
            _ => {
                vec![self.mesh_pipeline.view_layout_multisampled.clone()]
            }
        };
        if desc.vertex.shader_defs.contains(&"SKINNED".into()) {
            bind_group_layout.push(self.mesh_pipeline.skinned_mesh_layout.clone());
        } else {
            bind_group_layout.push(self.mesh_pipeline.mesh_layout.clone());
        };
        bind_group_layout.push(self.stencil_bind_group_layout.clone());

        desc.layout = bind_group_layout;
        desc.vertex.shader = STENCIL_SHADER_HANDLE.typed::<Shader>();
        desc.fragment = fragment_state(
            STENCIL_SHADER_HANDLE,
            "fragment",
            &[color_target(None)],
            &[],
        );
        desc.depth_stencil = None;

        Ok(desc)
    }
}

/// Make sure all 3d cameras have a [`MeshStencil`] [`RenderPhase`]
pub fn extract_stencil_phase(
    mut commands: Commands,
    cameras: Extract<Query<Entity, With<Camera3d>>>,
) {
    for entity in cameras.iter() {
        commands
            .get_or_spawn(entity)
            .insert(RenderPhase::<MeshStencil>::default());
    }
}

/// Create the StencilUniform for each mesh with an Outline component
pub fn extract_stencil_uniform(
    mut commands: Commands,
    outlines: Extract<Query<(Entity, &Outline)>>,
) {
    for (entity, outline) in &outlines {
        commands.get_or_spawn(entity).insert(StencilUniform {
            color: outline.color,
        });
    }
}

#[derive(Resource)]
pub struct StencilBindGroup {
    value: BindGroup,
}

/// Queues the creation of the stencil bind group
pub fn queue_stencil_bind_group(
    mut commands: Commands,
    stencil_pipeline: Res<StencilPipeline>,
    render_device: Res<RenderDevice>,
    uniforms: Res<ComponentUniforms<StencilUniform>>,
) {
    let Some(uniform) = uniforms.binding() else {
        return;
    };

    let stencil_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: Some("stencil_bind_group"),
        layout: &stencil_pipeline.stencil_bind_group_layout,
        entries: &bind_group_entries![
            0 => uniform.clone(),
        ],
    });

    commands.insert_resource(StencilBindGroup {
        value: stencil_bind_group,
    });
}

/// Add any visible entity with a mesh and an [`Outline`] to the stencil_phase
pub fn queue_mesh_stencil(
    stencil_draw_functions: Res<DrawFunctions<MeshStencil>>,
    stencil_pipeline: Res<StencilPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<StencilPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    render_meshes: Res<RenderAssets<Mesh>>,
    outline_meshes: Query<(Entity, &Handle<Mesh>, &MeshUniform), With<Outline>>,
    mut views: Query<(
        &ExtractedView,
        &mut VisibleEntities,
        &mut RenderPhase<MeshStencil>,
    )>,
    msaa: Res<Msaa>,
) {
    let draw_mesh_stencil = stencil_draw_functions
        .read()
        .get_id::<DrawMeshStencil>()
        .unwrap();

    for (view, visible_entities, mut stencil_phase) in views.iter_mut() {
        let view_matrix = view.transform.compute_matrix();
        let inv_view_row_2 = view_matrix.inverse().row(2);

        let view_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

        for visible_entity in visible_entities.entities.iter().copied() {
            let Ok((entity, mesh_handle, mesh_uniform)) = outline_meshes.get(visible_entity) else {
                continue;
            };
            let Some(mesh) = render_meshes.get(mesh_handle) else {
                continue;
            };

            let key = MeshPipelineKey::from_primitive_topology(mesh.primitive_topology) | view_key;

            let Ok(pipeline) = pipelines.specialize(&pipeline_cache, &stencil_pipeline, key, &mesh.layout) else {
                continue;
            };

            stencil_phase.add(MeshStencil {
                entity,
                pipeline,
                draw_function: draw_mesh_stencil,
                distance: inv_view_row_2.dot(mesh_uniform.transform.col(3)),
            });
        }
    }
}
