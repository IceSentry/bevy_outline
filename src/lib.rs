#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

mod blur_pipeline;
pub mod node;
mod stencil_phase;
mod utils;

use bevy::{
    asset::load_internal_asset,
    core_pipeline::core_3d,
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::ExtractedCamera,
        extract_component::{ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin},
        render_graph::RenderGraph,
        render_resource::{
            AddressMode, BindGroupLayout, BindGroupLayoutDescriptor, BindingType, BlendState,
            BufferBindingType, CachedRenderPipelineId, Extent3d, FilterMode, PipelineCache,
            Sampler, SamplerBindingType, SamplerDescriptor, ShaderType, SpecializedRenderPipelines,
            TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
            TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, CachedTexture, TextureCache},
        Extract, RenderApp, RenderStage,
    },
};
use blur_pipeline::{BlurDirection, BlurPipeline, BlurPipelineKey, BlurType};
use utils::{color_target, RenderPipelineDescriptorBuilder};

use crate::{blur_pipeline::BlurUniform, node::OutlineNode, stencil_phase::MeshStencilPlugin};

const BLUR_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 14687827633551304793);

const COMBINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 13593741836324854485);

#[derive(Component, Clone, Copy, Default)]
pub struct Outline {
    pub color: Color,
}

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

pub struct OutlinePlugin;
impl Plugin for OutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, BLUR_SHADER_HANDLE, "blur.wgsl", Shader::from_wgsl);
        load_internal_asset!(
            app,
            COMBINE_SHADER_HANDLE,
            "combine.wgsl",
            Shader::from_wgsl
        );

        app.add_plugin(ExtractComponentPlugin::<Outline>::default())
            .add_plugin(ExtractComponentPlugin::<OutlineSettings>::default())
            .add_plugin(UniformComponentPlugin::<BlurUniform>::default())
            .add_plugin(UniformComponentPlugin::<CombineSettingsUniform>::default())
            .add_plugin(MeshStencilPlugin);

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<BlurPipeline>()
            .init_resource::<SpecializedRenderPipelines<BlurPipeline>>()
            .init_resource::<OutlineMeta>()
            .add_system_to_stage(RenderStage::Extract, extract_outline_settings)
            .add_system_to_stage(RenderStage::Prepare, prepare_outline_textures)
            .add_system_to_stage(RenderStage::Prepare, prepare_blur_pipelines);

        {
            let outline_node = OutlineNode::new(&mut render_app.world);
            let mut graph = render_app.world.resource_mut::<RenderGraph>();
            let draw_3d_graph = graph.get_sub_graph_mut(core_3d::graph::NAME).unwrap();

            draw_3d_graph.add_node(graph::node::OUTLINE_PASS, outline_node);

            draw_3d_graph
                .add_slot_edge(
                    draw_3d_graph.input_node().unwrap().id,
                    graph::input::VIEW_ENTITY,
                    graph::node::OUTLINE_PASS,
                    OutlineNode::IN_VIEW,
                )
                .unwrap();

            draw_3d_graph
                .add_node_edge(core_3d::graph::node::MAIN_PASS, graph::node::OUTLINE_PASS)
                .unwrap();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutlineType {
    BoxBlur,
    GaussianBlur,
    MaxFilter,
    Jfa,
}

impl Default for OutlineType {
    fn default() -> Self {
        OutlineType::BoxBlur
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OutlineSettings {
    // The size or thickness of the outline, higher numbers will create wider outlines
    pub size: f32,
    // The intensity of the outline. Only useful for blurred outlines. Does nothing for other types of outline.
    pub intensity: f32,
    pub outline_type: OutlineType,
}

impl ExtractComponent for OutlineSettings {
    type Query = &'static Self;

    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<Self::Query>) -> Self {
        *item
    }
}
#[derive(Component, ShaderType, Clone)]
struct CombineSettingsUniform {
    intensity: f32,
}

#[derive(Component)]
pub struct StencilTexture(CachedTexture);

#[derive(Component)]
struct BlurredOutlineTextures {
    vertical_blur_texture: CachedTexture,
    horizontal_blur_texture: CachedTexture,
}

#[derive(Resource)]
struct OutlineMeta {
    sampler: Sampler,
    combine_bind_group_layout: BindGroupLayout,
    combine_pipeline: CachedRenderPipelineId,
}

impl FromWorld for OutlineMeta {
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
                    // intensity
                    3 => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(CombineSettingsUniform::min_size()),
                    },
                ],
            });

        let mut pipeline_cache = world.resource_mut::<PipelineCache>();

        let combine_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::fullscreen()
                .label("combine_pipeline")
                .fragment(
                    COMBINE_SHADER_HANDLE,
                    "combine",
                    // Additive blending
                    &[color_target(Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING))],
                    &[],
                )
                .layout(vec![combine_bind_group_layout.clone()])
                .build(),
        );

        Self {
            sampler,
            combine_bind_group_layout,
            combine_pipeline,
        }
    }
}

fn extract_outline_settings(
    mut commands: Commands,
    cameras: Extract<Query<(Entity, &Camera, &OutlineSettings), With<Camera3d>>>,
) {
    for (entity, camera, settings) in cameras.iter() {
        if let (Some((origin, _)), Some(size), Some(target_size)) = (
            camera.physical_viewport_rect(),
            camera.physical_viewport_size(),
            camera.physical_target_size(),
        ) {
            commands
                .get_or_spawn(entity)
                .insert(BlurUniform {
                    size: settings.size,
                    dims: Vec2::ONE / size.as_vec2(),
                    viewport: UVec4::new(origin.x, origin.y, size.x, size.y).as_vec4()
                        / UVec4::new(target_size.x, target_size.y, target_size.x, target_size.y)
                            .as_vec4(),
                })
                .insert(CombineSettingsUniform {
                    intensity: settings.intensity,
                })
                .insert(*settings);
        }
    }
}

#[derive(Component)]
struct BlurPipelines {
    vertical_blur_pipeline_id: CachedRenderPipelineId,
    horizontal_blur_pipeline_id: CachedRenderPipelineId,
}

fn prepare_blur_pipelines(
    mut commands: Commands,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlurPipeline>>,
    blur_pipeline: Res<BlurPipeline>,
    views: Query<(Entity, &OutlineSettings)>,
) {
    for (entity, settings) in &views {
        let blur_type = match settings.outline_type {
            OutlineType::BoxBlur => BlurType::Box,
            OutlineType::GaussianBlur { .. } => BlurType::Gaussian,
            _ => continue,
        };

        let vertical_blur_pipeline_id = pipelines.specialize(
            &mut pipeline_cache,
            &blur_pipeline,
            BlurPipelineKey {
                blur_type,
                direction: BlurDirection::Vertical,
            },
        );
        let horizontal_blur_pipeline_id = pipelines.specialize(
            &mut pipeline_cache,
            &blur_pipeline,
            BlurPipelineKey {
                blur_type,
                direction: BlurDirection::Horizontal,
            },
        );

        commands.entity(entity).insert(BlurPipelines {
            vertical_blur_pipeline_id,
            horizontal_blur_pipeline_id,
        });
    }
}

/// Prepares the textures used to render the outline
fn prepare_outline_textures(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &ExtractedCamera, &OutlineSettings)>,
) {
    for (entity, camera, settings) in &cameras {
        let Some(UVec2 { x, y }) = camera.physical_viewport_size else {
            continue;
        };

        let base_desc = TextureDescriptor {
            label: None,
            size: Extent3d {
                width: x,
                height: y,
                depth_or_array_layers: 1,
            },
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

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(StencilTexture(stencil_texture));

        match settings.outline_type {
            OutlineType::BoxBlur | OutlineType::GaussianBlur => {
                let vertical_blur_texture = texture_cache.get(
                    &render_device,
                    TextureDescriptor {
                        label: Some("vertical_blur_output"),
                        ..base_desc
                    },
                );
                let horizontal_blur_texture = texture_cache.get(
                    &render_device,
                    TextureDescriptor {
                        label: Some("horizontal_blur_output"),
                        ..base_desc
                    },
                );

                entity_commands.insert(BlurredOutlineTextures {
                    vertical_blur_texture,
                    horizontal_blur_texture,
                });
            }
            OutlineType::MaxFilter => todo!(),
            OutlineType::Jfa => todo!(),
        }
    }
}
