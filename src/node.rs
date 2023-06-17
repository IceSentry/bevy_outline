use bevy::{
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex},
        render_graph::{Node, RenderGraphContext, SlotInfo, SlotType},
        render_phase::RenderPhase,
        render_resource::{
            BindGroup, BindGroupDescriptor, BindingResource, LoadOp, Operations, PipelineCache,
            RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
        },
        renderer::{RenderContext, RenderDevice},
        texture::CachedTexture,
        view::ViewTarget,
    },
};

use crate::{
    bind_group_entries, blur_pipeline::BlurPipeline, stencil_phase::MeshStencil, BlurPipelines,
    BlurUniform, BlurredOutlineTextures, CombineSettingsUniform, MaxFilterSettingsUniform,
    OutlineSettings, OutlineType, StencilTexture,
};

use super::OutlineMeta;

/// Render node for drawing blurred outlines of selected meshes
pub struct OutlineNode {
    query: QueryState<(
        &'static ViewTarget,
        &'static RenderPhase<MeshStencil>,
        &'static BlurredOutlineTextures,
        &'static StencilTexture,
        &'static DynamicUniformIndex<BlurUniform>,
        &'static DynamicUniformIndex<CombineSettingsUniform>,
        &'static DynamicUniformIndex<MaxFilterSettingsUniform>,
        Option<&'static BlurPipelines>,
        &'static OutlineSettings,
    )>,
}

impl OutlineNode {
    pub const IN_VIEW: &'static str = "view";

    pub fn new(world: &mut World) -> OutlineNode {
        OutlineNode {
            query: QueryState::new(world),
        }
    }
}

impl Node for OutlineNode {
    fn input(&self) -> Vec<SlotInfo> {
        vec![SlotInfo::new(Self::IN_VIEW, SlotType::Entity)]
    }

    fn update(&mut self, world: &mut World) {
        self.query.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), bevy::render::render_graph::NodeRunError> {
        let view_entity = graph.get_input_entity(Self::IN_VIEW)?;
        let Ok((
            view_target,
            stencil_phase,
            blur_textures,
            stencil_texture,
            blur_uniform_index,
            intensity_uniform_index,
            max_filter_settings_uniform_index,
            blur_pipelines,
            settings,
        )) = self.query.get_manual(world, view_entity) else {
            return Ok(());
        };

        let pipelines = world.resource::<OutlineMeta>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let render_device = world.resource::<RenderDevice>();
        let blur_pipeline = world.resource::<BlurPipeline>();
        let Some(blur_uniforms) = world.resource::<ComponentUniforms<BlurUniform>>().binding() else {
            return Ok(());
        };
        let Some(combine_settings_uniforms) = world.resource::<ComponentUniforms<CombineSettingsUniform>>().binding() else {
            return Ok(());
        };
        let Some(max_filter_settings_uniforms) = world.resource::<ComponentUniforms<MaxFilterSettingsUniform>>().binding() else {
            return Ok(());
        };

        let (Some(combine_pipeline), Some(max_filter_pipeline)) = (
            pipeline_cache.get_render_pipeline(pipelines.combine_pipeline),
            pipeline_cache.get_render_pipeline(pipelines.max_filter_pipeline)
        ) else {
            return Ok(());
        };

        // General algorithm:
        // 1. Generate a stencil buffer of all the meshes with an outline component
        // 2. Vertical blur on the stencil buffer
        // 3. Horizontal blur on the vertical blur buffer
        // 4. Combine the final texture with the view_target

        // Draw stencil of all the entities with outlines
        draw_stencil(
            stencil_texture,
            render_context,
            world,
            stencil_phase,
            view_entity,
        );

        // TODO figure out how to downsample

        match settings.outline_type {
            OutlineType::BoxBlur | OutlineType::GaussianBlur => {
                let Some(blur_pipelines) = blur_pipelines else {
                    return Ok(());
                };

                let (Some(vertical_blur_pipeline), Some(horizontal_blur_pipeline)) = (
                    pipeline_cache.get_render_pipeline(blur_pipelines.vertical_blur_pipeline_id),
                    pipeline_cache.get_render_pipeline(blur_pipelines.horizontal_blur_pipeline_id),
                ) else {
                    return Ok(());
                };

                // TODO since only the texture changes, we should have a separate bind group for it
                // This means we could also reuse parts of it for both direction
                let blur_bind_group = |label, texture: &CachedTexture| {
                    render_device.create_bind_group(&BindGroupDescriptor {
                        label: Some(&format!("{label}_bind_group")),
                        layout: &blur_pipeline.layout,
                        entries: &bind_group_entries![
                            0 => BindingResource::TextureView(&texture.default_view),
                            1 => BindingResource::Sampler(&pipelines.sampler),
                            2 => blur_uniforms.clone(),
                        ],
                    })
                };
                blur_pass(
                    render_context,
                    vertical_blur_pipeline,
                    blur_bind_group("vertical_blur", &stencil_texture.texture),
                    blur_uniform_index,
                    &blur_textures.vertical_blur_texture,
                );

                let horizontal_bind_group =
                    blur_bind_group("horizontal_blur", &blur_textures.vertical_blur_texture);
                blur_pass(
                    render_context,
                    horizontal_blur_pipeline,
                    horizontal_bind_group.clone(),
                    blur_uniform_index,
                    &blur_textures.horizontal_blur_texture,
                );

                if let OutlineType::GaussianBlur = settings.outline_type {
                    // This essentially re-runs the blur on the already blurred texture.
                    // This makes it possible to have wider outlines.
                    // Using only a single step generates a lot of artifacts when using large sizes.

                    let vertical_bind_group =
                        blur_bind_group("vertical_blur", &blur_textures.horizontal_blur_texture);

                    for _ in 0..3 {
                        blur_pass(
                            render_context,
                            vertical_blur_pipeline,
                            vertical_bind_group.clone(),
                            blur_uniform_index,
                            &blur_textures.vertical_blur_texture,
                        );
                        blur_pass(
                            render_context,
                            horizontal_blur_pipeline,
                            horizontal_bind_group.clone(),
                            blur_uniform_index,
                            &blur_textures.horizontal_blur_texture,
                        );
                    }
                }
            }
            OutlineType::MaxFilter => {
                let max_filter_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                    label: Some("max_filter_bind_group"),
                    layout: &pipelines.max_filter_bind_group_layout,
                    entries: &bind_group_entries![
                        0 => BindingResource::TextureView(&stencil_texture.texture.default_view),
                        1 => BindingResource::Sampler(&pipelines.sampler),
                        2 => max_filter_settings_uniforms.clone(),
                    ],
                });
                max_filter_pass(
                    render_context,
                    &blur_textures.horizontal_blur_texture,
                    max_filter_pipeline,
                    max_filter_bind_group,
                    max_filter_settings_uniform_index,
                );
            }
            OutlineType::Jfa => todo!(),
        }

        // final combine pass
        let combine_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_combine_bind_group"),
            layout: &pipelines.combine_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::Sampler(&pipelines.sampler),
                1 => BindingResource::TextureView(&stencil_texture.texture.default_view),
                2 => BindingResource::TextureView(&blur_textures.horizontal_blur_texture.default_view),
                3 => combine_settings_uniforms.clone(),
            ],
        });
        combine_pass(
            render_context,
            combine_pipeline,
            combine_bind_group,
            view_target,
            intensity_uniform_index,
        );

        Ok(())
    }
}

fn max_filter_pass(
    render_context: &mut RenderContext,
    texture: &CachedTexture,
    max_filter_pipeline: &RenderPipeline,
    max_filter_bind_group: BindGroup,
    max_filter_settings_uniform_index: &DynamicUniformIndex<MaxFilterSettingsUniform>,
) {
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("max_filter_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &texture.default_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::NONE.into()),
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    });

    pass.set_render_pipeline(max_filter_pipeline);
    pass.set_bind_group(
        0,
        &max_filter_bind_group,
        &[max_filter_settings_uniform_index.index()],
    );
    pass.draw(0..3, 0..1);
}

fn draw_stencil(
    stencil_texture: &StencilTexture,
    render_context: &mut RenderContext,
    world: &World,
    stencil_phase: &RenderPhase<MeshStencil>,
    view_entity: Entity,
) {
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("outline_stencil_pass"),
        color_attachments: &[stencil_texture.get_color_attachment()],
        depth_stencil_attachment: None,
    });
    stencil_phase.render(&mut pass, world, view_entity);
}

fn blur_pass(
    render_context: &mut RenderContext,
    pipeline: &RenderPipeline,
    bind_group: BindGroup,
    blur_uniform_index: &DynamicUniformIndex<BlurUniform>,
    texture: &CachedTexture,
) {
    let mut blur_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("outline_blur_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &texture.default_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::NONE.into()),
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    });

    blur_pass.set_render_pipeline(pipeline);
    blur_pass.set_bind_group(0, &bind_group, &[blur_uniform_index.index()]);
    blur_pass.draw(0..3, 0..1);
}

fn combine_pass(
    render_context: &mut RenderContext,
    pipeline: &RenderPipeline,
    bind_group: BindGroup,
    view_target: &ViewTarget,
    intensity_uniform_index: &DynamicUniformIndex<CombineSettingsUniform>,
) {
    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("outline_combine_pass"),
        color_attachments: &[Some(view_target.get_unsampled_color_attachment(
            Operations {
                load: LoadOp::Load,
                store: true,
            },
        ))],
        depth_stencil_attachment: None,
    });

    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group, &[intensity_uniform_index.index()]);
    pass.draw(0..3, 0..1);
}
