use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;

use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::expansion_processing::process_expansion_fronts;
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::*;

/// Main game update system - orchestrates all subsystems with tracing
#[tracing::instrument(skip_all)]
pub fn update_game(
    mut board: ResMut<Board>,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    tile_change_writer: MessageWriter<TileChangeMessage>,
    player_map: Res<PlayerEntityMap>,
) {
    // FixedUpdate runs at 10hz automatically, no need for manual timer

    // 1. Check for eliminations and update troop generation
    check_eliminations_and_update_troops(&mut players, &mut expansions, &mut commands, &text_query);

    let pool = ComputeTaskPool::get();

    // 2. AI: Assign troops to expansion fronts and log active fronts
    assign_and_log_expansions(&board, &mut players, &mut expansions, &pool);

    // 3. Process all expansion fronts and move borders
    // Note: Borders are now updated incrementally inside process_expansion_fronts
    process_expansion_fronts(
        &mut board,
        &mut players,
        &mut expansions,
        tile_change_writer,
        &player_map,
    );

    // 4. Clear expansion fronts for pairs that no longer share a border and refund troops
    clear_disconnected_fronts(&board, &mut expansions, &mut players);
}
