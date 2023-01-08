use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayout, BlendState, ColorTargetState, ColorWrites, FragmentState,
            MultisampleState, PrimitiveState, RenderPipelineDescriptor, TextureFormat, VertexState,
        },
        texture::BevyDefault,
    },
};

pub fn color_target(blend: Option<BlendState>) -> ColorTargetState {
    ColorTargetState {
        format: TextureFormat::bevy_default(),
        blend,
        write_mask: ColorWrites::ALL,
    }
}

pub fn fragment_state(
    shader: HandleUntyped,
    entry_point: &'static str,
    targets: &[ColorTargetState],
) -> Option<FragmentState> {
    Some(FragmentState {
        entry_point: entry_point.into(),
        shader: shader.typed::<Shader>(),
        shader_defs: vec![],
        targets: targets.iter().map(|target| Some(target.clone())).collect(),
    })
}

#[macro_export]
macro_rules! bind_group_entries {
    ($($index:expr => $res:expr,)*) => {
        [$(
            bevy::render::render_resource::BindGroupEntry {
                binding: $index,
                resource: $res,
            },
        )*]
    };
}

#[macro_export]
macro_rules! bind_group_layout_entries {
    ($($index:expr => $ty:expr,)*) => {
        [$(
            bevy::render::render_resource::BindGroupLayoutEntry {
                binding: $index,
                ty: $ty,
                visibility: bevy::render::render_resource::ShaderStages::all(),
                count: None
            },
        )*]
    };
}

pub struct RenderPipelineDescriptorBuilder {
    desc: RenderPipelineDescriptor,
}

impl RenderPipelineDescriptorBuilder {
    #[allow(unused)]
    pub fn default(vertex_state: VertexState) -> RenderPipelineDescriptorBuilder {
        Self {
            desc: RenderPipelineDescriptor {
                fragment: None,
                vertex: vertex_state,
                label: None,
                layout: None,
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
            },
        }
    }

    pub fn fullscreen() -> RenderPipelineDescriptorBuilder {
        Self {
            desc: RenderPipelineDescriptor {
                fragment: None,
                vertex: fullscreen_shader_vertex_state(),
                label: None,
                layout: None,
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
            },
        }
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.desc.label = Some(label.into());
        self
    }

    pub fn fragment(
        mut self,
        shader: HandleUntyped,
        entry_point: &'static str,
        targets: &[ColorTargetState],
    ) -> Self {
        self.desc.fragment = fragment_state(shader, entry_point, targets);
        self
    }

    pub fn layout(mut self, layouts: Vec<BindGroupLayout>) -> Self {
        self.desc.layout = Some(layouts);
        self
    }

    pub fn build(self) -> RenderPipelineDescriptor {
        self.desc
    }
}
