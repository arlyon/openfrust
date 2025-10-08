use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

const LABEL_INTERVAL: usize = 256; // Show coordinate every 256 pixels

/// Resource holding the handle to our dynamic map texture
#[derive(Resource)]
pub struct MapTexture(pub Handle<Image>);

/// Creates and initializes the map texture at startup
pub fn setup_map_texture(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    board: Res<Board>,
    player_colors: Res<PlayerColorMap>,
) {
    // Create a new image asset with the full board dimensions
    // We'll directly manipulate its pixel data to draw the map
    let mut image = Image::new_fill(
        Extent3d {
            width: BOARD_WIDTH as u32,
            height: BOARD_HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255], // Initial black color
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Iterate through the game board and paint the initial state onto the image buffer
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let owner = board.get(x, y).owner() as usize;
            let color = player_colors.0[owner];

            // Set the pixel color at (x, y)
            image.set_color_at(x as u32, y as u32, color).ok();
        }
    }

    // Add the image to the asset server and get a handle to it
    let handle = images.add(image);

    // Store the handle in our resource so the update system can access it
    commands.insert_resource(MapTexture(handle.clone()));

    // Spawn a single sprite to display our entire map texture
    commands.spawn(Sprite {
        image: handle,
        // Scale the sprite to match our world dimensions
        custom_size: Some(Vec2::new(
            (BOARD_WIDTH as f32) * TILE_SIZE,
            (BOARD_HEIGHT as f32) * TILE_SIZE,
        )),
        ..default()
    });

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

/// Updates the map texture when tiles change ownership
/// This system listens for TileChangeMessage events and updates only the affected pixels
pub fn update_map_texture(
    mut tile_change_reader: MessageReader<TileChangeMessage>,
    map_texture: Res<MapTexture>,
    mut images: ResMut<Assets<Image>>,
    color_map: Res<PlayerColorMap>,
) {
    // Get mutable access to the image asset
    let Some(image) = images.get_mut(&map_texture.0) else {
        return;
    };

    // Process each tile change event
    for message in tile_change_reader.read() {
        let new_color = color_map.0[message.new_owner];

        // Update the pixel color at the message's coordinates
        image.set_color_at(message.x as u32, message.y as u32, new_color).ok();
    }
}
