//! Cursor-based player ID query system using GPU compute
//!
//! This module provides an asynchronous workflow for querying which player
//! owns the tile under the cursor. It uses a separate one_shot ComputeWorker
//! that shares the board_in buffer with the main ExpansionWorker.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_app_compute::prelude::*;

use crate::map::GameMap;
use crate::systems::gpu::PlayerIdWorker;

/// Resource to hold the current query request state.
/// Updated by a system that tracks the cursor position.
#[derive(Resource, Default, Debug)]
pub struct CursorIDQuery {
    /// The integer board coordinate we want to query
    pub board_coord: Option<UVec2>,
}

/// Resource to hold the final result from the GPU.
/// The UI can read from this every frame without worrying about GPU state.
#[derive(Resource, Default, Debug)]
pub struct CursorIDResult {
    pub player_id: Option<u32>,
}

/// System 1: Track cursor and convert its position to a board coordinate
pub fn update_cursor_query(
    mut cursor_query: ResMut<CursorIDQuery>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map: Res<GameMap>,
) {
    let Some(primary_window) = window_query.iter().next() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };

    if let Some(screen_pos) = primary_window.cursor_position() {
        // Convert screen position to world position
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) {
            // Convert world position to board coordinates (0,0 is top-left)
            let half_width = map.width() as f32 / 2.0;
            let half_height = map.height() as f32 / 2.0;

            let x = (world_pos.x + half_width).floor();
            let y = (half_height - world_pos.y).floor();

            if x >= 0.0 && x < map.width() as f32 && y >= 0.0 && y < map.height() as f32 {
                let new_coord = UVec2::new(x as u32, y as u32);
                // Only update if the coordinate has changed to avoid redundant dispatches
                if cursor_query.board_coord != Some(new_coord) {
                    trace!(x = new_coord.x, y = new_coord.y, "new board coord");
                    cursor_query.board_coord = Some(new_coord);
                }
                return;
            }
        } else {
            warn!("Could not convert screen position to world position");
        }
    }

    // If cursor is outside the window or the board, clear the query
    cursor_query.board_coord = None;
}

/// System 2: Dispatch the compute shader when a new query is requested
pub fn dispatch_id_query(
    query_request: Res<CursorIDQuery>,
    mut worker: ResMut<AppComputeWorker<PlayerIdWorker>>,
) {
    // Only run if the query request has changed since last time we checked
    if !query_request.is_changed() {
        return;
    }

    // Don't dispatch a new query if the GPU is already busy with the last one
    if worker.state.is_running() {
        return;
    }

    if let Some(board_coord) = query_request.board_coord {
        // Write the new coordinate to the GPU uniform buffer
        if worker
            .try_write_slice("target_coord", &[board_coord.x, board_coord.y])
            .is_err()
        {
            warn!("Could not write to GPU uniform buffer");
        } else {
            // Execute the compute shader
            worker.execute();
        }
    }
}

/// System 3: Check for and collect the result from the GPU when it's ready
pub fn process_id_query_result(
    mut result: ResMut<CursorIDResult>,
    worker: Res<AppComputeWorker<PlayerIdWorker>>,
) {
    // The `one_shot` worker will be `ready()` for one frame after the GPU finishes
    if worker.ready() {
        const NOT_FOUND: u32 = 0xFFFFFFFF;
        let result_vec = worker.read_vec::<u32>("result_id");
        let id = result_vec.first().copied().unwrap_or(NOT_FOUND);

        if id == NOT_FOUND {
            result.player_id = None;
        } else {
            result.player_id = Some(id);
        }
    }
}

/// Marker component for the player info panel UI
#[derive(Component)]
pub struct PlayerInfoPanel;

/// Setup the player info panel UI in the bottom right corner
pub fn setup_player_info_panel(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(10.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            PlayerInfoPanel,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("No player selected"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Update the player info panel with the currently selected player's data
pub fn update_player_info_panel(
    result: Res<CursorIDResult>,
    players: Query<&crate::types::PlayerData, With<crate::types::Alive>>,
    expansions: Res<crate::types::ActiveExpansions>,
    sim_manager: Res<crate::systems::SimManager>,
    panel_query: Query<Entity, With<PlayerInfoPanel>>,
    mut commands: Commands,
    player_map: Res<crate::types::PlayerEntityMap>,
) {
    // Only update when the result changes
    if !result.is_changed() {
        return;
    }

    let Some(panel_entity) = panel_query.iter().next() else {
        return;
    };

    // Clear existing children
    commands.entity(panel_entity).despawn_children();

    // Build the UI content
    commands.entity(panel_entity).with_children(|parent| {
        if let Some(player_id) = result.player_id {
            // Get player entity from the map
            let player_entity = player_map.0.get(player_id as usize).and_then(|e| *e);

            if let Some(entity) = player_entity {
                if let Ok(player_data) = players.get(entity) {
                    // Player header
                    parent.spawn((
                        Text::new(format!("Player {} ({})", player_data.id, player_data.char)),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(player_data.color),
                    ));

                    // Troops and tiles
                    parent.spawn((
                        Text::new(format!("Troops: {}", player_data.troops)),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    parent.spawn((
                        Text::new(format!("Tiles: {}", player_data.tile_count)),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    // Get adjacency data for borders
                    let adjacency = sim_manager.frame_manager.get_readable_adjacency();
                    let mut borders = Vec::new();

                    for other_id in 0..crate::NUM_ENTITIES {
                        let other_player_id = crate::types::PlayerId::new_unchecked(other_id);
                        if other_player_id != player_data.id {
                            let idx = crate::types::ActiveExpansions::pair_index(
                                player_data.id,
                                other_player_id,
                            );
                            // Check if this pair is in the adjacency matrix
                            let word_idx = idx / 32;
                            let bit_idx = idx % 32;
                            if word_idx < adjacency.len() {
                                let is_adjacent = (adjacency[word_idx] & (1 << bit_idx)) != 0;
                                if is_adjacent {
                                    borders.push(other_player_id);
                                }
                            }
                        }
                    }

                    // Display borders
                    if !borders.is_empty() {
                        parent.spawn((
                            Text::new(format!("\nBorders ({}):", borders.len())),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.8, 0.8)),
                        ));

                        for border in borders.iter().take(10) {
                            // Limit to first 10
                            parent.spawn((
                                Text::new(format!("  Player {}", border)),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                            ));
                        }

                        if borders.len() > 10 {
                            parent.spawn((
                                Text::new(format!("  ... and {} more", borders.len() - 10)),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                            ));
                        }
                    } else {
                        parent.spawn((
                            Text::new("\nNo borders"),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.6, 0.6, 0.6)),
                        ));
                    }

                    // Display active fronts
                    let mut active_fronts = Vec::new();
                    for other_id in 0..crate::NUM_ENTITIES {
                        let other_player_id = crate::types::PlayerId::new_unchecked(other_id);
                        if other_player_id != player_data.id {
                            let troops = expansions.get_net_troops(player_data.id, other_player_id);
                            if troops != 0 {
                                active_fronts.push((other_player_id, troops));
                            }
                        }
                    }

                    if !active_fronts.is_empty() {
                        parent.spawn((
                            Text::new(format!("\nActive Fronts ({}):", active_fronts.len())),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.8, 0.8)),
                        ));

                        for (target, troops) in active_fronts.iter().take(10) {
                            let color = if *troops > 0 {
                                Color::srgb(0.3, 1.0, 0.3) // Green for attacking
                            } else {
                                Color::srgb(1.0, 0.3, 0.3) // Red for defending
                            };

                            let action = if *troops > 0 { "→" } else { "←" };

                            parent.spawn((
                                Text::new(format!(
                                    "  {} P{}: {} troops",
                                    action,
                                    target,
                                    troops.abs()
                                )),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(color),
                            ));
                        }

                        if active_fronts.len() > 10 {
                            parent.spawn((
                                Text::new(format!("  ... and {} more", active_fronts.len() - 10)),
                                TextFont {
                                    font_size: 12.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                            ));
                        }
                    }
                } else {
                    parent.spawn((
                        Text::new(format!("Player {} (eliminated)", player_id)),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    ));
                }
            } else {
                parent.spawn((
                    Text::new(format!("Player {} (wilderness)", player_id)),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.4)),
                ));
            }
        } else {
            parent.spawn((
                Text::new("No player selected"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        }
    });
}
