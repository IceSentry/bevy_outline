pub mod node;
pub mod plugin;

use bevy::{
    pbr::{DrawMesh, MeshPipeline, MeshPipelineKey, SetMeshBindGroup, SetMeshViewBindGroup},
    prelude::*,
    reflect::TypeUuid,
    render::{
        mesh::InnerMeshVertexBufferLayout,
        render_phase::{
            CachedRenderPipelinePhaseItem, DrawFunctionId, EntityPhaseItem, PhaseItem,
            SetItemPipeline,
        },
        render_resource::{
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            RenderPipelineDescriptor, SpecializedMeshPipeline, SpecializedMeshPipelineError,
            TextureFormat,
        },
        texture::{BevyDefault, CachedTexture},
    },
    utils::{FixedState, FloatOrd, Hashed},
};

pub const STENCIL_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 10400755559809425757);

#[derive(Component)]
pub struct StencilTexture {
    pub texture: CachedTexture,
}

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
        desc.fragment = Some(FragmentState {
            shader: STENCIL_SHADER_HANDLE.typed::<Shader>(),
            shader_defs: vec![],
            entry_point: "fragment".into(),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
        });
        desc.depth_stencil = None;

        Ok(desc)
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
