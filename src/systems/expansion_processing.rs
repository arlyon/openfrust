use bevy::prelude::*;

use crate::types::*;

/// Legacy CPU-based expansion processing - no longer used with GPU orchestrator
#[allow(dead_code)]
pub fn process_expansion_fronts(
    _board: &mut Board,
    _players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    _expansions: &mut ActiveExpansions,
    _player_map: &PlayerEntityMap,
) {
    // This system has been replaced by GPU orchestrator
    unreachable!("Legacy CPU system - use GPU orchestrator instead");
}
