use bevy::{
    prelude::*, reflect::TypePath, render::render_resource::AsBindGroup,
    render::storage::ShaderStorageBuffer, shader::ShaderRef, sprite_render::Material2d,
};

/// Custom material for rendering territory borders
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct BorderMaterial {
    /// The board data storage buffer containing raw tile data (u32 array)
    #[storage(0, read_only)]
    pub board_data: Handle<ShaderStorageBuffer>,

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

    /// Map terrain data (packed u8 MapTiles, 4 per u32)
    #[storage(5, read_only)]
    pub map_terrain: Handle<ShaderStorageBuffer>,

    /// Global time for animations, like water effects
    #[uniform(6)]
    pub time: f32,

    /// Enable/disable fancy water animation (0 = disabled, 1 = enabled)
    #[uniform(7)]
    pub enable_water_animation: u32,

    /// Enable/disable player rendering - colors and borders (0 = disabled, 1 = enabled)
    #[uniform(8)]
    pub enable_players: u32,

    /// Enable/disable sphere projection (0 = disabled, 1 = enabled)
    #[uniform(9)]
    pub enable_sphere_projection: u32,

    /// Pre-computed distance field texture (distance to nearest land)
    #[texture(10)]
    #[sampler(11)]
    pub distance_texture: Handle<Image>,
}

impl Material2d for BorderMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/border.wgsl".into()
    }
}
