use bevy::prelude::*;

use crate::types::{Board, PlayerData, Alive, ActiveExpansions, PlayerEntityMap};

/// Legacy CPU-based game update - no longer used with GPU orchestrator
#[allow(dead_code)]
pub fn game_update(
    _board: ResMut<Board>,
    _players: Query<(Entity, &mut PlayerData), With<Alive>>,
    _expansions: ResMut<ActiveExpansions>,
    _player_map: Res<PlayerEntityMap>,
) {
    // This system has been replaced by GPU orchestrator
    unreachable!("Legacy CPU system - use GPU orchestrator instead");
}
