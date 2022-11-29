use bevy::{
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_graph::{Node, RenderGraphContext, SlotInfo, SlotType},
        render_phase::{DrawFunctions, PhaseItem, RenderPhase, TrackedRenderPass},
        render_resource::{
            LoadOp, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor,
        },
        renderer::RenderContext,
        view::ViewTarget,
    },
};

use super::{MeshStencil, OutlineBindGroups, OutlinePipelines, StencilTexture};

/// Render graph node for producing stencils from meshes.
pub struct StencilNode {
    query: QueryState<(
        &'static ViewTarget,
        &'static ExtractedCamera,
        &'static RenderPhase<MeshStencil>,
        &'static StencilTexture,
        &'static OutlineBindGroups,
    )>,
}

impl StencilNode {
    pub const IN_VIEW: &'static str = "view";

    pub fn new(world: &mut World) -> StencilNode {
        StencilNode {
            query: QueryState::new(world),
        }
    }
}

impl Node for StencilNode {
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
        let Ok((view_target, camera, stencil_phase, textures, bind_groups)) = self.query.get_manual(world, view_entity) else {
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

        // Draw a stencil of all the entities with outlines
        {
            let mut stencil_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_stencil_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &textures.stencil_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::BLACK.into()),
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
                            view: &textures.vertical_blur_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::BLACK.into()),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            vertical_blur_pass.set_render_pipeline(vertical_blur_pipeline);
            vertical_blur_pass.set_bind_group(0, &bind_groups.blur_bind_group, &[]);
            if let Some(viewport) = camera.viewport.as_ref() {
                vertical_blur_pass.set_camera_viewport(viewport);
            }
            vertical_blur_pass.draw(0..3, 0..1);
        }
        // horizontal blur
        {
            let mut vertical_blur_pass =
                TrackedRenderPass::new(render_context.command_encoder.begin_render_pass(
                    &RenderPassDescriptor {
                        label: Some("outline_horizontal_blur_pass"),
                        color_attachments: &[Some(RenderPassColorAttachment {
                            view: &textures.horizontal_blur_texture.default_view,
                            resolve_target: None,
                            ops: Operations {
                                load: LoadOp::Clear(Color::BLACK.into()),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    },
                ));

            vertical_blur_pass.set_render_pipeline(horizontal_blur_pipeline);
            vertical_blur_pass.set_bind_group(0, &bind_groups.blur_bind_group, &[]);
            if let Some(viewport) = camera.viewport.as_ref() {
                vertical_blur_pass.set_camera_viewport(viewport);
            }
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
            combine_pass.set_bind_group(0, &bind_groups.combine_bind_group, &[]);
            if let Some(viewport) = camera.viewport.as_ref() {
                combine_pass.set_camera_viewport(viewport);
            }
            combine_pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
