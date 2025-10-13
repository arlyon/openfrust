//! Player info panel UI plugin
//!
//! This module provides a UI panel that displays detailed information about the player
//! whose territory is currently under the cursor.
//!
//! ## Usage
//!
//! Add the [`PlayerInfoPanelPlugin`] to your app:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use openfrust::systems::plugins::player_info_panel::PlayerInfoPanelPlugin;
//! App::new()
//!     .add_plugins(PlayerInfoPanelPlugin)
//!     .run();
//! ```
//!
//! The panel will automatically display information about the player under the cursor,
//! including troops, tile count, borders, and active fronts.

use bevy::prelude::*;

use crate::shaders::compute::CursorIDResult;

/// Marker component for the player info panel UI
#[derive(Component)]
pub struct PlayerInfoPanel;

const FONT: &str = "fonts/IBMPlexMono-Regular.ttf";
const FONT_BOLD: &str = "fonts/IBMPlexMono-Medium.ttf";
const BASIC_FONT_SIZE: f32 = 12.0;
const HEADER_FONT_SIZE: f32 = 16.0;

/// Setup the player info panel UI in the bottom right corner
pub fn setup_player_info_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
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
                    font_size: BASIC_FONT_SIZE,
                    font: asset_server.load(FONT),
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
    sim_manager: Res<crate::SimManager>,
    panel_query: Query<Entity, With<PlayerInfoPanel>>,
    mut commands: Commands,
    player_map: Res<crate::types::PlayerEntityMap>,
    asset_server: Res<AssetServer>,
) {
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
                            font_size: HEADER_FONT_SIZE,
                            font: asset_server.load(FONT_BOLD),
                            ..default()
                        },
                        TextColor(player_data.color),
                    ));

                    // Troops and tiles
                    parent.spawn((
                        Text::new(format!("Troops: {}", player_data.troops)),
                        TextFont {
                            font_size: BASIC_FONT_SIZE,
                            font: asset_server.load(FONT),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));

                    parent.spawn((
                        Text::new(format!("Tiles: {}", player_data.tile_count)),
                        TextFont {
                            font_size: BASIC_FONT_SIZE,
                            font: asset_server.load(FONT),
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
                                font_size: BASIC_FONT_SIZE,
                                font: asset_server.load(FONT),
                                ..default()
                            },
                            TextColor(Color::srgb(0.8, 0.8, 0.8)),
                        ));

                        for border in borders.iter().take(10) {
                            // Limit to first 10
                            parent.spawn((
                                Text::new(format!("  Player {}", border)),
                                TextFont {
                                    font_size: BASIC_FONT_SIZE,
                                    font: asset_server.load(FONT),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.7, 0.7, 0.7)),
                            ));
                        }

                        if borders.len() > 10 {
                            parent.spawn((
                                Text::new(format!("  ... and {} more", borders.len() - 10)),
                                TextFont {
                                    font_size: BASIC_FONT_SIZE,
                                    font: asset_server.load(FONT),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.5, 0.5, 0.5)),
                            ));
                        }
                    } else {
                        parent.spawn((
                            Text::new("\nNo borders"),
                            TextFont {
                                font_size: BASIC_FONT_SIZE,
                                font: asset_server.load(FONT),
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
                                font_size: BASIC_FONT_SIZE,
                                font: asset_server.load(FONT),
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
                                    font_size: BASIC_FONT_SIZE,
                                    font: asset_server.load(FONT),
                                    ..default()
                                },
                                TextColor(color),
                            ));
                        }

                        if active_fronts.len() > 10 {
                            parent.spawn((
                                Text::new(format!("  ... and {} more", active_fronts.len() - 10)),
                                TextFont {
                                    font_size: BASIC_FONT_SIZE,
                                    font: asset_server.load(FONT),
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
                            font_size: BASIC_FONT_SIZE,
                            font: asset_server.load(FONT),
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.5, 0.5)),
                    ));
                }
            } else {
                parent.spawn((
                    Text::new(format!("Player {} (wilderness)", player_id)),
                    TextFont {
                        font_size: BASIC_FONT_SIZE,
                        font: asset_server.load(FONT),
                        ..default()
                    },
                    TextColor(Color::srgb(0.6, 0.6, 0.4)),
                ));
            }
        } else {
            parent.spawn((
                Text::new("No player selected"),
                TextFont {
                    font_size: BASIC_FONT_SIZE,
                    font: asset_server.load(FONT),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        }
    });
}

/// Plugin that manages the player info panel UI.
///
/// This plugin sets up a UI panel in the bottom-right corner that displays
/// detailed information about the player whose territory is under the cursor.
///
/// The plugin adds:
/// - [`setup_player_info_panel`] system in [`Startup`] to create the UI
/// - [`update_player_info_panel`] system in [`Update`] to refresh the panel content
///
/// The panel shows:
/// - Player ID, character, and color
/// - Troop count and tile count
/// - List of neighboring players (borders)
/// - Active military fronts with troop allocations
///
/// ## Example
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use openfrust::systems::plugins::player_info_panel::PlayerInfoPanelPlugin;
/// App::new()
///     .add_plugins(PlayerInfoPanelPlugin)
///     .run();
/// ```
pub struct PlayerInfoPanelPlugin;

impl Plugin for PlayerInfoPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_player_info_panel)
            .add_systems(Update, update_player_info_panel);
    }
}
