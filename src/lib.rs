#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

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
        extract_component::{
            ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin,
        },
        render_graph::RenderGraph,
        render_resource::{
            AddressMode, BindGroup, BindGroupDescriptor, BindGroupLayout,
            BindGroupLayoutDescriptor, BindingResource, BindingType, BlendComponent, BlendFactor,
            BlendOperation, BlendState, BufferBindingType, CachedRenderPipelineId, Extent3d,
            FilterMode, PipelineCache, Sampler, SamplerBindingType, SamplerDescriptor, ShaderType,
            TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
            TextureViewDimension,
        },
        renderer::RenderDevice,
        texture::{BevyDefault, CachedTexture, TextureCache},
        Extract, RenderApp, RenderStage,
    },
};
use utils::{color_target, RenderPipelineDescriptorBuilder};

use crate::{node::OutlineNode, stencil_phase::MeshStencilPlugin};

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

pub struct BlurredOutlinePlugin;
impl Plugin for BlurredOutlinePlugin {
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
            .add_plugin(MeshStencilPlugin);

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .init_resource::<OutlinePipelines>()
            .add_system_to_stage(RenderStage::Extract, extract_blur_uniform)
            .add_system_to_stage(RenderStage::Prepare, prepare_outline_resources);

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

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OutlineSettings {
    pub size: f32,
}

impl ExtractComponent for OutlineSettings {
    type Query = &'static Self;

    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<Self::Query>) -> Self {
        *item
    }
}

#[derive(Component, ShaderType, Clone)]
struct BlurUniform {
    size: f32,
    dims: Vec2,
    viewport: Vec4,
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

#[derive(Resource)]
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
                    2 => BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: Some(BlurUniform::min_size()),
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
                    &[color_target(Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent::REPLACE,
                    }))],
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

fn extract_blur_uniform(
    mut commands: Commands,
    cameras: Extract<Query<(Entity, &Camera, Option<&OutlineSettings>), With<Camera3d>>>,
) {
    for (entity, camera, settings) in cameras.iter() {
        if let (Some((origin, _)), Some(size), Some(target_size)) = (
            camera.physical_viewport_rect(),
            camera.physical_viewport_size(),
            camera.physical_target_size(),
        ) {
            commands.get_or_spawn(entity).insert(BlurUniform {
                size: settings.map(|s| s.size).unwrap_or(8.0),
                dims: Vec2::ONE / size.as_vec2(),
                viewport: UVec4::new(origin.x, origin.y, size.x, size.y).as_vec4()
                    / UVec4::new(target_size.x, target_size.y, target_size.x, target_size.y)
                        .as_vec4(),
            });
        }
    }
}

/// Prepares the textures and the bind groups used to render the outline
fn prepare_outline_resources(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    pipelines: Res<OutlinePipelines>,
    mut texture_cache: ResMut<TextureCache>,
    cameras: Query<(Entity, &ExtractedCamera)>,
    uniforms: Res<ComponentUniforms<BlurUniform>>,
) {
    let Some(uniform) = uniforms.binding() else {
        return;
    };

    for (entity, camera) in &cameras {
        let Some(UVec2 { x, y }) = camera.physical_viewport_size else {
            continue;
        };

        // TODO make this configurable
        let size = Extent3d {
            width: x,  // (x / 2).max(1),
            height: y, // (y / 2).max(1),
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
                2 => uniform.clone(),
            ],
        });

        let horizontal_blur_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_horizontal_blur_bind_group"),
            layout: &pipelines.blur_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::TextureView(&vertical_blur_texture.default_view),
                1 => BindingResource::Sampler(&pipelines.sampler),
                2 => uniform.clone(),
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
