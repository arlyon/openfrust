use bevy::prelude::*;

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

/// Render system to update tile colors based on TileChangeMessages
/// Message-driven approach: only updates tiles that actually changed
#[tracing::instrument(skip_all)]
pub fn update_tiles(
    mut tile_change_reader: MessageReader<TileChangeMessage>,
    tile_map: Res<TileEntityMap>,
    color_map: Res<PlayerColorMap>,
    mut sprite_query: Query<&mut Sprite>,
) {
    let _span = tracing::info_span!("update_tiles").entered();

    for message in tile_change_reader.read() {
        // 1. Get the entity for the changed tile in O(1)
        let tile_entity = tile_map.0[message.y][message.x];

        // 2. Get the sprite component for that specific entity
        if let Ok(mut sprite) = sprite_query.get_mut(tile_entity) {
            // 3. Get the new color in O(1)
            sprite.color = color_map.0[message.new_owner];
        }
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

            // Calculate center using cached coordinate sums (O(1) instead of O(board_size))
            if player.tile_count > 0 {
                let center_x = player.sum_x as f32 / player.tile_count as f32;
                let center_y = player.sum_y as f32 / player.tile_count as f32;

                let pos_x = (center_x - BOARD_WIDTH as f32 / 2.0) * TILE_SIZE;
                let pos_y = (BOARD_HEIGHT as f32 / 2.0 - center_y) * TILE_SIZE;

                transform.translation.x = pos_x;
                transform.translation.y = pos_y;
            }
        }
    }
}
