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
            BufferBindingType, CachedRenderPipelineId, Extent3d, FilterMode, LoadOp, Operations,
            PipelineCache, RenderPassColorAttachment, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderType, SpecializedRenderPipelines, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
            TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, CachedTexture, TextureCache},
        Extract, RenderApp, RenderSet,
    },
};
use blur_pipeline::{BlurDirection, BlurPipeline, BlurPipelineKey, BlurType};
use utils::{color_target, RenderPipelineDescriptorBuilder};

use crate::{blur_pipeline::BlurUniform, node::OutlineNode, stencil_phase::MeshStencilPlugin};

const BLUR_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 14687827633551304793);

const COMBINE_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 13593741836324854485);

const MAX_FILTER_SHADER_HANDLE: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 3759434788503552836);

#[derive(Component, Clone, Copy, Default, ExtractComponent)]
pub struct Outline {
    pub color: Color,
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
        load_internal_asset!(
            app,
            MAX_FILTER_SHADER_HANDLE,
            "max_filter.wgsl",
            Shader::from_wgsl
        );

        app.add_plugin(ExtractComponentPlugin::<Outline>::default())
            .add_plugin(ExtractComponentPlugin::<OutlineSettings>::default())
            .add_plugin(UniformComponentPlugin::<BlurUniform>::default())
            .add_plugin(UniformComponentPlugin::<CombineSettingsUniform>::default())
            .add_plugin(UniformComponentPlugin::<MaxFilterSettingsUniform>::default())
            .add_plugin(MeshStencilPlugin);

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<BlurPipeline>()
            .init_resource::<SpecializedRenderPipelines<BlurPipeline>>()
            .init_resource::<OutlineMeta>()
            .add_system(extract_outline_settings.in_schedule(ExtractSchedule))
            .add_system(prepare_outline_textures.in_set(RenderSet::Prepare))
            .add_system(prepare_blur_pipelines.in_set(RenderSet::Prepare));

        {
            let outline_node = OutlineNode::new(&mut render_app.world);
            let mut graph = render_app.world.resource_mut::<RenderGraph>();
            let draw_3d_graph = graph.get_sub_graph_mut(core_3d::graph::NAME).unwrap();

            draw_3d_graph.add_node(graph::node::OUTLINE_PASS, outline_node);

            draw_3d_graph.add_slot_edge(
                draw_3d_graph.input_node().id,
                graph::input::VIEW_ENTITY,
                graph::node::OUTLINE_PASS,
                OutlineNode::IN_VIEW,
            );

            draw_3d_graph.add_node_edge(core_3d::graph::node::MAIN_PASS, graph::node::OUTLINE_PASS);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum OutlineType {
    #[default]
    BoxBlur,
    GaussianBlur,
    MaxFilter,
    Jfa,
}

#[derive(Component, Clone, Copy, Debug, Default, ExtractComponent)]
pub struct OutlineSettings {
    // The size or thickness of the outline, higher numbers will create wider outlines
    pub size: f32,
    // The intensity of the outline. Only useful for blurred outlines. Does nothing for other types of outline.
    pub intensity: f32,
    pub outline_type: OutlineType,
}

#[derive(Component, ShaderType, Clone)]
struct CombineSettingsUniform {
    intensity: f32,
}

#[derive(Component, ShaderType, Clone)]
struct MaxFilterSettingsUniform {
    size: f32,
    dims: Vec2,
    viewport: Vec4,
}

#[derive(Component)]
pub struct StencilTexture {
    texture: CachedTexture,
    texture_sampled: Option<CachedTexture>,
}

impl StencilTexture {
    fn get_color_attachment(&self) -> Option<RenderPassColorAttachment<'_>> {
        let ops = Operations {
            load: LoadOp::Clear(Color::NONE.into()),
            store: true,
        };
        match self.texture_sampled.as_ref() {
            Some(CachedTexture { default_view, .. }) => Some(RenderPassColorAttachment {
                view: default_view,
                resolve_target: Some(&self.texture.default_view),
                ops,
            }),
            None => Some(RenderPassColorAttachment {
                view: &self.texture.default_view,
                resolve_target: None,
                ops,
            }),
        }
    }
}

#[derive(Component)]
struct BlurredOutlineTextures {
    vertical_blur_texture: CachedTexture,
    horizontal_blur_texture: CachedTexture,
}

#[derive(Resource)]
struct OutlineMeta {
    sampler: Sampler,
    max_filter_bind_group_layout: BindGroupLayout,
    max_filter_pipeline: CachedRenderPipelineId,
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

        let max_filter_bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("max_filter_bind_group_layout"),
                entries: &bind_group_layout_entries![
                    // input texture
                    0 => texture,
                    // sampler
                    1 => BindingType::Sampler(SamplerBindingType::Filtering),
                    // uniform
                    2 => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(MaxFilterSettingsUniform::min_size()),
                    },
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
                    // intensity
                    3 => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(CombineSettingsUniform::min_size()),
                    },
                ],
            });

        let pipeline_cache = world.resource::<PipelineCache>();

        let max_filter_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::fullscreen()
                .label("max_filter_pipeline".into())
                .fragment(
                    MAX_FILTER_SHADER_HANDLE,
                    "fragment",
                    &[color_target(None)],
                    &[],
                )
                .layout(vec![max_filter_bind_group_layout.clone()])
                .build(),
        );

        let combine_pipeline = pipeline_cache.queue_render_pipeline(
            RenderPipelineDescriptorBuilder::fullscreen()
                .label("combine_pipeline".into())
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
            max_filter_bind_group_layout,
            max_filter_pipeline,
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
            let viewport = UVec4::new(origin.x, origin.y, size.x, size.y).as_vec4()
                / UVec4::new(target_size.x, target_size.y, target_size.x, target_size.y).as_vec4();
            commands
                .get_or_spawn(entity)
                .insert(BlurUniform {
                    size: settings.size,
                    dims: Vec2::ONE / size.as_vec2(),
                    viewport,
                })
                .insert(CombineSettingsUniform {
                    intensity: settings.intensity,
                })
                .insert(MaxFilterSettingsUniform {
                    size: match settings.outline_type {
                        OutlineType::BoxBlur | OutlineType::GaussianBlur => settings.size / 2.0,
                        OutlineType::MaxFilter => settings.size,
                        OutlineType::Jfa => 0.0,
                    },
                    dims: Vec2::ONE / size.as_vec2(),
                    viewport,
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
    pipeline_cache: Res<PipelineCache>,
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
            &pipeline_cache,
            &blur_pipeline,
            BlurPipelineKey {
                blur_type,
                direction: BlurDirection::Vertical,
            },
        );
        let horizontal_blur_pipeline_id = pipelines.specialize(
            &pipeline_cache,
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
    msaa: Res<Msaa>,
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
            view_formats: &[],
        };

        let stencil_texture = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("stencil_output"),
                ..base_desc
            },
        );

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert(StencilTexture {
            texture: stencil_texture,
            texture_sampled: match msaa.samples() {
                1 => None,
                _ => Some(texture_cache.get(
                    &render_device,
                    TextureDescriptor {
                        label: Some("stencil_texture_multisampled"),
                        sample_count: msaa.samples(),
                        ..base_desc
                    },
                )),
            },
        });

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
            OutlineType::MaxFilter => {
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
            OutlineType::Jfa => todo!(),
        }
    }
}
