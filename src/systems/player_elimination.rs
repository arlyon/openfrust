use bevy::prelude::*;

use crate::types::*;
use crate::utils::count_tiles;

/// Handles player elimination and troop generation
#[tracing::instrument(skip_all)]
pub fn check_eliminations_and_update_troops(
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
    commands: &mut Commands,
    text_query: &Query<(Entity, &PlayerInfoText)>,
) -> Vec<(Entity, usize)> {
    let mut to_eliminate = Vec::new();

    for (entity, mut player) in players.iter_mut() {
        let tiles_owned = count_tiles(&board, player.id);

        if tiles_owned == 0 {
            bevy::log::warn!("Player {} has been eliminated!", player.id);
            to_eliminate.push((entity, player.id));
            continue;
        }

        // Calculate max troops based on territory (non-linear scaling)
        let max_troops = (2.0 * ((tiles_owned as f32).powf(0.6) * 1000.0 + 50000.0)) as u32;

        // Calculate troop growth with braking mechanism
        let base_growth = 10.0 + (player.troops as f32).powf(0.73) / 4.0;
        let braking_ratio = (1.0 - (player.troops as f32 / max_troops as f32)).max(0.0);
        let new_troops = (base_growth * braking_ratio) as u32;

        player.troops = (player.troops + new_troops).min(max_troops);

        bevy::log::debug!(
            "Player {} [{}]: {}/{} troops (+{})",
            player.id,
            tiles_owned,
            player.troops,
            max_troops,
            new_troops
        );
    }

    // Handle eliminations
    for (entity, player_id) in &to_eliminate {
        // Remove Alive marker
        commands.entity(*entity).remove::<Alive>();

        // Remove all expansion fronts to/from this player
        expansions.remove_player(*player_id);

        // Delete name tag
        for (text_entity, info) in text_query.iter() {
            if info.player_entity == *entity {
                commands.entity(text_entity).despawn();
            }
        }
    }

    to_eliminate
}
