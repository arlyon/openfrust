use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
    sprite_render::Material2d,
};

/// Custom material for rendering territory borders
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BorderMaterial {
    /// The map texture containing territory colors
    #[texture(0)]
    #[sampler(1)]
    pub map_texture: Handle<Image>,

    /// Color multiplier for borders (e.g., vec4(0.3, 0.3, 0.3, 1.0) for darker borders)
    #[uniform(2)]
    pub border_color: LinearRgba,

    /// Border thickness in pixels
    #[uniform(3)]
    pub border_thickness: f32,

    /// Size of the texture in pixels (width, height)
    #[uniform(4)]
    pub texture_size: Vec2,
}

impl Material2d for BorderMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/border.wgsl".into()
    }
}
