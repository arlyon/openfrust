use bevy::prelude::*;

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

/// Render system to update tile colors
#[tracing::instrument(skip_all)]
pub fn update_tiles(
    board: Res<Board>,
    players: Query<&PlayerData, With<Alive>>,
    mut query: Query<(&TileEntity, &mut Sprite)>,
) {
    if !board.is_changed() {
        return;
    }

    for (tile_entity, mut sprite) in query.iter_mut() {
        let owner = board.tiles[tile_entity.y][tile_entity.x].owner;
        sprite.color = if owner == NO_OWNER {
            Color::srgb(0.1, 0.1, 0.1)
        } else {
            players
                .iter()
                .find(|p| p.id == owner)
                .map(|p| p.color)
                .unwrap_or(Color::srgb(0.1, 0.1, 0.1))
        };
    }
}

/// Update player info text with troop counts and position at territory center
#[tracing::instrument(skip_all)]
pub fn update_player_info(
    board: Res<Board>,
    players: Query<(Entity, &PlayerData), (With<Alive>, Changed<PlayerData>)>,
    mut text_query: Query<(&crate::PlayerInfoText, &mut Text2d, &mut Transform)>,
) {
    if players.is_empty() && !board.is_changed() {
        return;
    }

    for (player_info, mut text, mut transform) in text_query.iter_mut() {
        if let Some((_, player)) = players
            .iter()
            .find(|(e, _)| *e == player_info.player_entity)
        {
            // Update text
            text.0 = format!("P{}: {}", player.id, player.troops);

            // Calculate center of player's territory
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut count = 0;

            for y in 0..BOARD_HEIGHT {
                for x in 0..BOARD_WIDTH {
                    if board.tiles[y][x].owner == player.id {
                        sum_x += x as f32;
                        sum_y += y as f32;
                        count += 1;
                    }
                }
            }

            if count > 0 {
                let center_x = sum_x / count as f32;
                let center_y = sum_y / count as f32;

                let pos_x = (center_x - BOARD_WIDTH as f32 / 2.0) * TILE_SIZE;
                let pos_y = (BOARD_HEIGHT as f32 / 2.0 - center_y) * TILE_SIZE;

                transform.translation.x = pos_x;
                transform.translation.y = pos_y;
            }
        }
    }
}
