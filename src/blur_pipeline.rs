use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayout, BindGroupLayoutDescriptor, BindingType, BufferBindingType,
            FragmentState, MultisampleState, PrimitiveState, RenderPipelineDescriptor,
            SamplerBindingType, ShaderType, SpecializedRenderPipeline, TextureSampleType,
            TextureViewDimension,
        },
        renderer::RenderDevice,
    },
};

use crate::{bind_group_layout_entries, utils::color_target, BLUR_SHADER_HANDLE};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum BlurDirection {
    Vertical,
    Horizontal,
}

impl std::fmt::Display for BlurDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlurDirection::Vertical => write!(f, "vertical"),
            BlurDirection::Horizontal => write!(f, "horizontal"),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum BlurType {
    Box,
    Gaussian,
}

#[derive(Component, ShaderType, Clone)]
pub struct BlurUniform {
    pub size: f32,
    pub dims: Vec2,
    pub viewport: Vec4,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct BlurPipelineKey {
    pub blur_type: BlurType,
    pub direction: BlurDirection,
}

#[derive(Resource)]
pub struct BlurPipeline {
    pub layout: BindGroupLayout,
}

impl FromWorld for BlurPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let texture = BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable: true },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        };

        let layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("blur_bind_group_layout"),
            entries: &bind_group_layout_entries![
                // input texture
                0 => texture,
                // sampler
                1 => BindingType::Sampler(SamplerBindingType::Filtering),
                // uniform
                2 => BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(BlurUniform::min_size()),
                },
            ],
        });

        BlurPipeline { layout }
    }
}

impl SpecializedRenderPipeline for BlurPipeline {
    type Key = BlurPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = vec![];

        match key.direction {
            BlurDirection::Vertical => shader_defs.push("VERTICAL".to_string()),
            BlurDirection::Horizontal => shader_defs.push("HORIZONTAL".to_string()),
        };

        match key.blur_type {
            BlurType::Box => shader_defs.push("BOX_BLUR".to_string()),
            BlurType::Gaussian => shader_defs.push("GAUSSIAN_BLUR".to_string()),
        }

        RenderPipelineDescriptor {
            label: Some(format!("{}_blur_pipeline", key.direction).into()),
            layout: Some(vec![self.layout.clone()]),
            vertex: fullscreen_shader_vertex_state(),
            fragment: Some(FragmentState {
                shader: BLUR_SHADER_HANDLE.typed(),
                shader_defs,
                entry_point: "fragment".into(),
                targets: vec![Some(color_target(None))],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
        }
    }
}
