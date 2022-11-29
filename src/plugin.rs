use bevy::{
    core_pipeline::core_3d,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_graph::RenderGraph,
        RenderApp,
    },
};

use crate::stencil_node::{node::StencilNode, plugin::StencilPassPlugin};

#[derive(Component, Clone, Copy)]
pub struct Outline;

impl ExtractComponent for Outline {
    type Query = &'static Self;

    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<Self::Query>) -> Self {
        *item
    }
}

mod graph {
    pub mod input {
        pub const VIEW_ENTITY: &str = "view_entity";
    }

    pub mod node {
        pub const STENCIL_PASS: &str = "stencil_pass";
    }
}

pub struct BlurredOutlinePlugin;
impl Plugin for BlurredOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(StencilPassPlugin)
            .add_plugin(ExtractComponentPlugin::<Outline>::default());

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        {
            let stencil_node = StencilNode::new(&mut render_app.world);
            let mut graph = render_app.world.resource_mut::<RenderGraph>();
            let draw_3d_graph = graph.get_sub_graph_mut(core_3d::graph::NAME).unwrap();

            draw_3d_graph.add_node(graph::node::STENCIL_PASS, stencil_node);

            draw_3d_graph
                .add_slot_edge(
                    draw_3d_graph.input_node().unwrap().id,
                    graph::input::VIEW_ENTITY,
                    graph::node::STENCIL_PASS,
                    StencilNode::IN_VIEW,
                )
                .unwrap();

            // MAIN_PASS -> STENCIL
            draw_3d_graph
                .add_node_edge(core_3d::graph::node::MAIN_PASS, graph::node::STENCIL_PASS)
                .unwrap();
        }
    }
}
