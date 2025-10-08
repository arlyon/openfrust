use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup,
    render::storage::ShaderStorageBuffer, shader::ShaderRef, sprite_render::Material2d,
};

/// Custom material for rendering territory borders
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BorderMaterial {
    /// The map texture containing raw tile data (R16Uint)
    /// No sampler needed - we use textureLoad instead of textureSample
    #[texture(0, sample_type = "u_int")]
    pub map_texture: Handle<Image>,

    /// Color multiplier for borders (e.g., vec4(0.3, 0.3, 0.3, 1.0) for darker borders)
    #[uniform(1)]
    pub border_color: LinearRgba,

    /// Border thickness in pixels
    #[uniform(2)]
    pub border_thickness: f32,

    /// Size of the texture in pixels (width, height)
    #[uniform(3)]
    pub texture_size: Vec2,

    /// Array of player colors for looking up by owner ID
    #[storage(4, read_only)]
    pub player_colors: Handle<ShaderStorageBuffer>,
}

impl Material2d for BorderMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/border.wgsl".into()
    }
}
