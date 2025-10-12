use bevy::prelude::*;

use crate::map::GameMap;
use crate::types::{LivingPlayerUpdate, PlayerData};
use crate::TILE_SIZE;

/// Update player info text with troop counts and position at territory center
#[tracing::instrument(skip_all)]
pub fn update_player_info(
    players: Query<(Entity, &PlayerData), LivingPlayerUpdate>,
    mut text_query: Query<(&crate::PlayerInfoText, &mut Text2d, &mut Transform)>,
    map: Res<GameMap>,
) {
    if players.is_empty() {
        return;
    }

    for (player_info, mut text, mut transform) in &mut text_query {
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

                let pos_x = (center_x - map.width() as f32 / 2.0) * TILE_SIZE;
                let pos_y = (map.height() as f32 / 2.0 - center_y) * TILE_SIZE;

                transform.translation.x = pos_x;
                transform.translation.y = pos_y;
            }
        }
    }
}
