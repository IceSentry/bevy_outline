use bevy::{
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayout, BindGroupLayoutDescriptor, BindingType, BufferBindingType,
            RenderPipelineDescriptor, SamplerBindingType, ShaderType, SpecializedRenderPipeline,
            TextureSampleType, TextureViewDimension,
        },
        renderer::RenderDevice,
    },
};

use crate::{
    bind_group_layout_entries,
    utils::{color_target, RenderPipelineDescriptorBuilder},
    BLUR_SHADER_HANDLE,
};

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
            BlurDirection::Vertical => shader_defs.push("VERTICAL".into()),
            BlurDirection::Horizontal => shader_defs.push("HORIZONTAL".into()),
        };

        match key.blur_type {
            BlurType::Box => shader_defs.push("BOX_BLUR".into()),
            BlurType::Gaussian => shader_defs.push("GAUSSIAN_BLUR".into()),
        }

        RenderPipelineDescriptorBuilder::fullscreen()
            .label(format!("{}_blur_pipeline", key.direction))
            .layout(vec![self.layout.clone()])
            .fragment(
                BLUR_SHADER_HANDLE,
                "fragment",
                &[color_target(None)],
                &shader_defs,
            )
            .build()
    }
}
