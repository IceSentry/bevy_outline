mod fullscreen_vertex_shader;
pub mod outline_node;

use bevy::{
    asset::load_internal_asset,
    core_pipeline::core_3d,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_graph::RenderGraph,
        RenderApp,
    },
};

use crate::{
    fullscreen_vertex_shader::FULLSCREEN_SHADER_HANDLE,
    outline_node::{node::OutlineNode, plugin::OutlineNodePlugin},
};

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
        pub const OUTLINE_PASS: &str = "stencil_pass";
    }
}

pub struct BlurredOutlinePlugin;
impl Plugin for BlurredOutlinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            FULLSCREEN_SHADER_HANDLE,
            "fullscreen_vertex_shader/fullscreen.wgsl",
            Shader::from_wgsl
        );

        app.add_plugin(OutlineNodePlugin)
            .add_plugin(ExtractComponentPlugin::<Outline>::default());

        let Ok(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        {
            let stencil_node = OutlineNode::new(&mut render_app.world);
            let mut graph = render_app.world.resource_mut::<RenderGraph>();
            let draw_3d_graph = graph.get_sub_graph_mut(core_3d::graph::NAME).unwrap();

            draw_3d_graph.add_node(graph::node::OUTLINE_PASS, stencil_node);

            draw_3d_graph
                .add_slot_edge(
                    draw_3d_graph.input_node().unwrap().id,
                    graph::input::VIEW_ENTITY,
                    graph::node::OUTLINE_PASS,
                    OutlineNode::IN_VIEW,
                )
                .unwrap();

            // MAIN_PASS -> STENCIL
            draw_3d_graph
                .add_node_edge(core_3d::graph::node::MAIN_PASS, graph::node::OUTLINE_PASS)
                .unwrap();
        }
    }
}
