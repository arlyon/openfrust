use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
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

/// Updates the map texture when tiles change ownership
/// This system listens for TileChangeMessage events and updates only the affected pixels
pub fn update_map_texture(
    mut tile_change_reader: MessageReader<TileChangeMessage>,
    map_texture: Res<MapTexture>,
    map_material: Res<MapMaterial>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BorderMaterial>>,
    color_map: Res<PlayerColorMap>,
) {
    let mut has_changes = false;

    // Get mutable access to the image asset
    let Some(image) = images.get_mut(&map_texture.0) else {
        return;
    };

    // Process each tile change event
    for message in tile_change_reader.read() {
        let new_color = color_map.0[message.new_owner];

        // Update the pixel color at the message's coordinates
        image
            .set_color_at(message.x as u32, message.y as u32, new_color)
            .ok();

        has_changes = true;
    }

    // Force material to update by touching it (triggers change detection)
    if has_changes {
        // Just accessing materials.get_mut triggers change detection
        materials.get_mut(&map_material.0);
    }
}
