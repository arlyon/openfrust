use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::storage::ShaderStorageBuffer;
use bevy::sprite_render::MeshMaterial2d;

use crate::systems::BorderMaterial;
use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

const LABEL_INTERVAL: usize = 256; // Show coordinate every 256 pixels

/// Resource holding the handle to our dynamic map texture
#[derive(Resource)]
pub struct MapTexture(pub Handle<Image>);

/// Resource holding the material handle so we can update it
#[derive(Resource)]
pub struct MapMaterial(pub Handle<BorderMaterial>);

/// Creates and initializes the map texture at startup
pub fn setup_map_texture(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BorderMaterial>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    board: Res<Board>,
    player_colors: Res<PlayerColorMap>,
) {
    // Create a new R16Uint data texture containing raw Tile data
    // The GPU will assemble the final image by looking up colors from the owner ID
    let mut image = Image::new_fill(
        Extent3d {
            width: BOARD_WIDTH as u32,
            height: BOARD_HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0], // Initial zero data (u16 = 2 bytes)
        TextureFormat::R16Uint,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Iterate through the game board and write raw tile data to the image buffer
    if let Some(data) = &mut image.data {
        for y in 0..BOARD_HEIGHT {
            for x in 0..BOARD_WIDTH {
                let tile = board.get(x, y);
                let tile_index = y * BOARD_WIDTH + x;
                let byte_index = tile_index * 2; // u16 = 2 bytes

                // Write the raw Tile(u16) data
                let bytes = tile.0.to_le_bytes();
                data[byte_index] = bytes[0];
                data[byte_index + 1] = bytes[1];
            }
        }
    }

    // Add the image to the asset server and get a handle to it
    let handle = images.add(image);

    // Store the handle in our resource so the update system can access it
    commands.insert_resource(MapTexture(handle.clone()));

    // Create a storage buffer containing player colors
    // Convert LinearRgba colors to Vec4 for the shader
    let color_data: Vec<[f32; 4]> = player_colors
        .0
        .iter()
        .map(|color| {
            let linear: LinearRgba = (*color).into();
            [linear.red, linear.green, linear.blue, linear.alpha]
        })
        .collect();

    let colors_buffer = buffers.add(ShaderStorageBuffer::new(
        bytemuck::cast_slice(&color_data),
        RenderAssetUsages::RENDER_WORLD,
    ));

    // Create a quad mesh that matches the map dimensions
    let mesh = meshes.add(Rectangle::new(
        BOARD_WIDTH as f32 * TILE_SIZE,
        BOARD_HEIGHT as f32 * TILE_SIZE,
    ));

    // Create the border material with the map texture
    let material_handle = materials.add(BorderMaterial {
        map_texture: handle,
        border_color: LinearRgba::new(0.3, 0.3, 0.3, 1.0), // Darker borders
        border_thickness: 1.0,
        texture_size: Vec2::new(BOARD_WIDTH as f32, BOARD_HEIGHT as f32),
        player_colors: colors_buffer,
    });

    // Store material handle for updates
    commands.insert_resource(MapMaterial(material_handle.clone()));

    // Spawn the map using our custom material
    commands.spawn((Mesh2d(mesh), MeshMaterial2d(material_handle)));

    // Spawn coordinate labels along the borders
    spawn_coordinate_labels(&mut commands);
}

/// Spawns text labels along the borders showing X and Y coordinates
fn spawn_coordinate_labels(commands: &mut Commands) {
    let half_width = (BOARD_WIDTH as f32 * TILE_SIZE) / 2.0;
    let half_height = (BOARD_HEIGHT as f32 * TILE_SIZE) / 2.0;

    // X-axis labels (top and bottom borders)
    let mut x = 0;
    while x <= BOARD_WIDTH {
        let x_pos = (x as f32 * TILE_SIZE) - half_width;

        // Top border
        commands.spawn((
            Text2d::new(format!("{}", x)),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(x_pos, half_height + 15.0, 1.0),
        ));

        // Bottom border
        commands.spawn((
            Text2d::new(format!("{}", x)),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(x_pos, -half_height - 15.0, 1.0),
        ));

        x += LABEL_INTERVAL;
    }

    // Y-axis labels (left and right borders)
    let mut y = 0;
    while y <= BOARD_HEIGHT {
        let y_pos = (y as f32 * TILE_SIZE) - half_height;

        // Left border
        commands.spawn((
            Text2d::new(format!("{}", y)),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(-half_width - 30.0, y_pos, 1.0),
        ));

        // Right border
        commands.spawn((
            Text2d::new(format!("{}", y)),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(half_width + 30.0, y_pos, 1.0),
        ));

        y += LABEL_INTERVAL;
    }
}

// Legacy rendering system - replaced by GPU-to-GPU copy
// NOTE: For now, the texture is still updated manually from the CPU board
// Future optimization: implement GPU-to-GPU copy using copy_to_texture.wgsl
