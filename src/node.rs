use bevy::{
    prelude::*,
    render::{
        extract_component::DynamicUniformIndex,
        render_graph::{Node, RenderGraphContext, SlotInfo, SlotType},
        render_phase::{DrawFunctions, PhaseItem, RenderPhase, TrackedRenderPass},
        render_resource::{
            LoadOp, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
        },
        renderer::RenderContext,
        view::ViewTarget,
    },
};

use crate::{BlurUniform, MeshStencil, OutlineResources};

use super::OutlinePipelines;

/// Render node for drawing blurred outlines of selected meshes
pub struct OutlineNode {
    query: QueryState<(
        &'static ViewTarget,
        &'static RenderPhase<MeshStencil>,
        &'static OutlineResources,
        &'static DynamicUniformIndex<BlurUniform>,
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
        let Ok((view_target, stencil_phase, resources, uniform_index)) = self.query.get_manual(world, view_entity) else {
            return Ok(());
        };

        let pipelines = world.resource::<OutlinePipelines>();
        let pipeline_cache = world.resource::<PipelineCache>();

        let (Some(vertical_blur_pipeline), Some(horizontal_blur_pipeline), Some(combine_pipeline)) = (
            pipeline_cache.get_render_pipeline(pipelines.vertical_blur_pipeline),
            pipeline_cache.get_render_pipeline(pipelines.horizontal_blur_pipeline),
            pipeline_cache.get_render_pipeline(pipelines.combine_pipeline)
        ) else {
            return Ok(());
        };

        // General algorithm:
        // Generate a stencil buffer of all the meshes with an outline component
        // Vertical blur on the stencil buffer
        // Horizontal blur on the vertical blur buffer
        // Combine the final texture with the view_targer

        // Draw a stencil of all the entities with outlines
        {
            let mut stencil_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_stencil_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &resources.stencil_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::NONE.into()),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            let draw_functions = world.resource::<DrawFunctions<MeshStencil>>();
            let mut draw_functions = draw_functions.write();
            for item in stencil_phase.items.iter() {
                draw_functions.get_mut(item.draw_function()).unwrap().draw(
                    world,
                    &mut stencil_pass,
                    view_entity,
                    item,
                );
            }
        }

        // vertical blur
        {
            let mut vertical_blur_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_vertical_blur_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &resources.vertical_blur_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::NONE.into()),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            vertical_blur_pass.set_render_pipeline(vertical_blur_pipeline);
            vertical_blur_pass.set_bind_group(
                0,
                &resources.vertical_blur_bind_group,
                &[uniform_index.index()],
            );
            vertical_blur_pass.draw(0..3, 0..1);
        }

        // horizontal blur
        {
            let mut vertical_blur_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_horizontal_blur_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &resources.horizontal_blur_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::NONE.into()),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            vertical_blur_pass.set_render_pipeline(horizontal_blur_pipeline);
            vertical_blur_pass.set_bind_group(
                0,
                &resources.horizontal_blur_bind_group,
                &[uniform_index.index()],
            );
            vertical_blur_pass.draw(0..3, 0..1);
        }

        // final combine pass
        {
            let mut combine_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_combine_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &view_target.view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Load,
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            combine_pass.set_render_pipeline(combine_pipeline);
            combine_pass.set_bind_group(0, &resources.combine_bind_group, &[]);
            combine_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
