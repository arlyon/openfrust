use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::storage::ShaderStorageBuffer;
use bevy::sprite_render::MeshMaterial2d;
use bevy_app_compute::prelude::*;

use crate::systems::{BorderMaterial, ExpansionWorker};
use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

const LABEL_INTERVAL: usize = 256; // Show coordinate every 256 pixels

/// Resource holding the material handle so we can update it
#[derive(Resource)]
pub struct MapMaterial(pub Handle<BorderMaterial>);

/// Creates and initializes the map rendering at startup
pub fn setup_map_texture(
    mut commands: Commands,
    worker: Res<AppComputeWorker<ExpansionWorker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BorderMaterial>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    player_colors: Res<PlayerColorMap>,
) {
    // Get the handle to the render buffer
    let board_handle = worker
        .get_storage_buffer_asset_handle("board_render")
        .expect("board_render storage buffer asset should exist")
        .clone();

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

    // Create the border material with the storage buffer handle
    let material_handle = materials.add(BorderMaterial {
        board_data: board_handle,
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
