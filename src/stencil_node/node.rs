use bevy::{
    prelude::*,
    render::{
        render_graph::{Node, RenderGraphContext, SlotInfo, SlotType},
        render_phase::{DrawFunctions, PhaseItem, RenderPhase, TrackedRenderPass},
        render_resource::{LoadOp, Operations, RenderPassColorAttachment, RenderPassDescriptor},
        renderer::RenderContext,
    },
};

use super::{MeshStencil, StencilTexture};

/// Render graph node for producing stencils from meshes.
pub struct StencilNode {
    query: QueryState<(&'static RenderPhase<MeshStencil>, &'static StencilTexture)>,
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
        let Ok((stencil_phase, textures)) = self.query.get_manual(world, view_entity) else {
            return Ok(());
        };

        let pass_raw = render_context
            .command_encoder
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("outline_stencil_render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &textures.texture.default_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK.into()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
        let mut pass = TrackedRenderPass::new(pass_raw);

        let draw_functions = world.resource::<DrawFunctions<MeshStencil>>();
        let mut draw_functions = draw_functions.write();
        for item in stencil_phase.items.iter() {
            draw_functions.get_mut(item.draw_function()).unwrap().draw(
                world,
                &mut pass,
                view_entity,
                item,
            );
        }

        Ok(())
    }
}
