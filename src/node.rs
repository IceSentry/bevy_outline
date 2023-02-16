use bevy::{
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex},
        render_graph::{Node, RenderGraphContext, SlotInfo, SlotType},
        render_phase::{DrawFunctions, PhaseItem, RenderPhase, TrackedRenderPass},
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
    BlurUniform, BlurredOutlineTextures, CombineSettingsUniform, OutlineSettings, OutlineType,
    StencilTexture,
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
        &'static BlurPipelines,
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

        let (Some(vertical_blur_pipeline), Some(horizontal_blur_pipeline), Some(combine_pipeline)) = (
            pipeline_cache.get_render_pipeline(blur_pipelines.vertical_blur_pipeline_id),
            pipeline_cache.get_render_pipeline(blur_pipelines.horizontal_blur_pipeline_id),
            pipeline_cache.get_render_pipeline(pipelines.combine_pipeline)
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

        // blur
        {
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
                blur_bind_group("vertical_blur", &stencil_texture.0),
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

        // final combine pass
        let combine_bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: Some("outline_combine_bind_group"),
            layout: &pipelines.combine_bind_group_layout,
            entries: &bind_group_entries![
                0 => BindingResource::Sampler(&pipelines.sampler),
                1 => BindingResource::TextureView(&stencil_texture.0.default_view),
                2 => BindingResource::TextureView(&blur_textures.horizontal_blur_texture.default_view),
                3 => combine_settings_uniforms.clone(),
            ],
        });
        combine_textures(
            render_context,
            combine_pipeline,
            combine_bind_group,
            view_target,
            intensity_uniform_index,
        );

        Ok(())
    }
}

fn draw_stencil(
    stencil_texture: &StencilTexture,
    render_context: &mut RenderContext,
    world: &World,
    stencil_phase: &RenderPhase<MeshStencil>,
    view_entity: Entity,
) {
    let pass_desc = RenderPassDescriptor {
        label: Some("outline_stencil_pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: &stencil_texture.0.default_view,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(Color::NONE.into()),
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    };
    let mut stencil_pass =
        TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(&pass_desc));

    let draw_functions = world.resource::<DrawFunctions<MeshStencil>>();
    let mut draw_functions = draw_functions.write();
    for item in &stencil_phase.items {
        draw_functions.get_mut(item.draw_function()).unwrap().draw(
            world,
            &mut stencil_pass,
            view_entity,
            item,
        );
    }
}

fn blur_pass(
    render_context: &mut RenderContext,
    pipeline: &RenderPipeline,
    bind_group: BindGroup,
    blur_uniform_index: &DynamicUniformIndex<BlurUniform>,
    texture: &CachedTexture,
) {
    let pass_desc = RenderPassDescriptor {
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
    };
    let mut blur_pass =
        TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(&pass_desc));

    blur_pass.set_render_pipeline(pipeline);
    blur_pass.set_bind_group(0, &bind_group, &[blur_uniform_index.index()]);
    blur_pass.draw(0..3, 0..1);
}

fn combine_textures(
    render_context: &mut RenderContext,
    pipeline: &RenderPipeline,
    bind_group: BindGroup,
    view_target: &ViewTarget,
    intensity_uniform_index: &DynamicUniformIndex<CombineSettingsUniform>,
) {
    let pass_desc = RenderPassDescriptor {
        label: Some("outline_combine_pass"),
        color_attachments: &[Some(view_target.get_unsampled_color_attachment(
            Operations {
                load: LoadOp::Load,
                store: true,
            },
        ))],
        depth_stencil_attachment: None,
    };
    let mut combine_pass =
        TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(&pass_desc));

    combine_pass.set_render_pipeline(pipeline);
    combine_pass.set_bind_group(0, &bind_group, &[intensity_uniform_index.index()]);
    combine_pass.draw(0..3, 0..1);
}
