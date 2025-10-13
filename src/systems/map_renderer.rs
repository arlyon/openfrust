use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::storage::ShaderStorageBuffer;
use bevy::sprite_render::MeshMaterial2d;
use bevy_app_compute::prelude::*;

use crate::TILE_SIZE;
use crate::map::GameMap;
use crate::shaders::BorderMaterial;
use crate::shaders::compute::ExpansionWorker;
use crate::types::{PlayerColorMap, RenderSettings};

const LABEL_INTERVAL: usize = 256; // Show coordinate every 256 pixels

/// Creates and initializes the map rendering at startup
pub fn setup_map_texture(
    mut commands: Commands,
    worker: Res<AppComputeWorker<ExpansionWorker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BorderMaterial>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    player_colors: Res<PlayerColorMap>,
    map: Res<GameMap>,
    render_settings: Res<RenderSettings>,
) {
    // Get the handle to the render buffer
    let board_handle = worker
        .get_storage_buffer_asset_handle("board_render")
        .expect("board_render storage buffer asset should exist")
        .clone();

    // Get the handle to the terrain buffer
    let terrain_handle = worker
        .get_storage_buffer_asset_handle("map_terrain")
        .expect("map_terrain storage buffer asset should exist")
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
        map.width() as f32 * TILE_SIZE,
        map.height() as f32 * TILE_SIZE,
    ));

    // Create the border material with the storage buffer handle
    let material_handle = materials.add(BorderMaterial {
        board_data: board_handle,
        border_color: LinearRgba::new(0.3, 0.3, 0.3, 1.0), // Darker borders
        border_thickness: 1.0,
        texture_size: Vec2::new(map.width() as f32, map.height() as f32),
        player_colors: colors_buffer,
        map_terrain: terrain_handle,
        time: 0.0,
        enable_water_animation: if render_settings.enable_water_animation {
            1
        } else {
            0
        },
        enable_players: if render_settings.enable_players { 1 } else { 0 },
        enable_sphere_projection: 0,
    });

    // Spawn the map using our custom material
    commands.spawn((Mesh2d(mesh), MeshMaterial2d(material_handle)));

    // Spawn coordinate labels along the borders
    spawn_coordinate_labels(&mut commands, &map);
}

/// Spawns text labels along the borders showing X and Y coordinates
fn spawn_coordinate_labels(commands: &mut Commands, map: &GameMap) {
    let board_width = map.width() as usize;
    let board_height = map.height() as usize;
    let half_width = (board_width as f32 * TILE_SIZE) / 2.0;
    let half_height = (board_height as f32 * TILE_SIZE) / 2.0;

    // X-axis labels (top and bottom borders)
    let mut x = 0;
    while x <= board_width {
        let x_pos = (x as f32 * TILE_SIZE) - half_width;

        // Top border
        commands.spawn((
            Text2d::new(format!("{x}")),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(x_pos, half_height + 15.0, 1.0),
        ));

        // Bottom border
        commands.spawn((
            Text2d::new(format!("{x}")),
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
    while y <= board_height {
        let y_pos = (y as f32 * TILE_SIZE) - half_height;

        // Left border
        commands.spawn((
            Text2d::new(format!("{y}")),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
            Transform::from_xyz(-half_width - 30.0, y_pos, 1.0),
        ));

        // Right border
        commands.spawn((
            Text2d::new(format!("{y}")),
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

/// Updates the time uniform in all BorderMaterial assets to animate water
pub fn update_water_animation_time(
    time: Res<Time>,
    mut materials: ResMut<Assets<BorderMaterial>>,
    render_settings: Res<RenderSettings>,
) {
    // Only update time if water animation is enabled
    if render_settings.enable_water_animation {
        for (_, material) in materials.iter_mut() {
            material.time = time.elapsed_secs();
        }
    }
}

/// Syncs RenderSettings changes to all BorderMaterial instances
pub fn sync_render_settings_to_materials(
    render_settings: Res<RenderSettings>,
    mut materials: ResMut<Assets<BorderMaterial>>,
) {
    if render_settings.is_changed() {
        for (_, material) in materials.iter_mut() {
            material.enable_water_animation = if render_settings.enable_water_animation {
                1
            } else {
                0
            };
            material.enable_players = if render_settings.enable_players { 1 } else { 0 };
        }
    }
}
