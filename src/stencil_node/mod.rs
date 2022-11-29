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
            AddressMode, BindGroup, BindGroupLayout, BindGroupLayoutDescriptor,
            BindGroupLayoutEntry, BindingType, CachedRenderPipelineId, ColorTargetState,
            ColorWrites, FilterMode, FragmentState, MultisampleState, PipelineCache,
            PrimitiveState, RenderPipelineDescriptor, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, SpecializedMeshPipeline, SpecializedMeshPipelineError,
            TextureFormat, TextureSampleType, TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, CachedTexture},
    },
    utils::{FixedState, FloatOrd, Hashed},
};

use crate::fullscreen_vertex_shader::fullscreen_shader_vertex_state;

pub const STENCIL_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 10400755559809425757);

const BLUR_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 929599476923908);

#[derive(Component)]
pub struct StencilTexture {
    pub stencil_texture: CachedTexture,
    pub vertical_blur_texture: CachedTexture,
    pub horizontal_blur_texture: CachedTexture,
}

#[derive(Component)]
pub struct OutlineBindGroups {
    blur_bind_group: BindGroup,
}

pub struct OutlinePipelines {
    sampler: Sampler,
    blur_bind_group_layout: BindGroupLayout,
    horizontal_blur_pipeline: CachedRenderPipelineId,
    vertical_blur_pipeline: CachedRenderPipelineId,
}

impl FromWorld for OutlinePipelines {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let sampler = render_device.create_sampler(&SamplerDescriptor {
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            ..Default::default()
        });
        let blur_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("bloom_downsampling_bind_group_layout"),
                entries: &[
                    // input texture
                    BindGroupLayoutEntry {
                        binding: 0,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        visibility: ShaderStages::FRAGMENT,
                        count: None,
                    },
                    // sampler
                    BindGroupLayoutEntry {
                        binding: 1,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        visibility: ShaderStages::FRAGMENT,
                        count: None,
                    },
                ],
            });

        let mut pipeline_cache = world.resource_mut::<PipelineCache>();

        let vertical_blur_pipeline =
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("vertical_blur_pipeline".into()),
                layout: Some(vec![blur_bind_group_layout.clone()]),
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: BLUR_SHADER_HANDLE.typed::<Shader>(),
                    shader_defs: vec![],
                    entry_point: "vertical_blur".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
            });

        let horizontal_blur_pipeline =
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("horizontal_blur_pipeline".into()),
                layout: Some(vec![blur_bind_group_layout.clone()]),
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader: BLUR_SHADER_HANDLE.typed::<Shader>(),
                    shader_defs: vec![],
                    entry_point: "horizontal_blur".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::bevy_default(),
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
            });

        Self {
            sampler,
            blur_bind_group_layout,
            vertical_blur_pipeline,
            horizontal_blur_pipeline,
        }
    }
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
