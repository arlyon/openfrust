use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;

use super::border_calculation::update_borders_incremental;
use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::gpu::{ExpansionWorker, Front};
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, EXPANSION_RATE_BASE};

/// GPU-accelerated game update system - orchestrates CPU and GPU work
#[tracing::instrument(skip_all)]
pub fn gpu_orchestrator(
    mut board: ResMut<Board>,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    mut tile_change_writer: MessageWriter<TileChangeMessage>,
    player_map: Res<PlayerEntityMap>,
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
) {
    // Synchronization point: wait for GPU to finish previous tick
    if !worker.ready() {
        return; // GPU still working, skip this tick to maintain determinism
    }

    // --- GPU has completed previous tick, read results and update CPU state ---
    // After swap, board_in contains the previous frame's output
    let board_data = worker.read_vec::<u32>("board_in");
    process_gpu_results(
        &board_data,
        &mut board,
        &mut players,
        &mut tile_change_writer,
        &player_map,
    );

    // --- CPU Phase: High-level game logic ---

    // 1. Check for eliminations and update troop generation
    check_eliminations_and_update_troops(&mut players, &mut expansions, &mut commands, &text_query);

    let pool = ComputeTaskPool::get();

    // 2. AI: Assign troops to expansion fronts
    assign_and_log_expansions(&board, &mut players, &mut expansions, pool);

    // 3. Prepare GPU data: Convert active fronts to GPU format
    let gpu_fronts = prepare_gpu_fronts(&expansions);

    // 4. Write data to GPU buffers
    worker.write_slice("fronts", &gpu_fronts);

    // Reset atomic counters for this tick
    worker.write_slice("conquer_counters", &vec![0u32; crate::NUM_PAIRS]);

    // 5. Clear expansion fronts for disconnected borders and refund troops
    clear_disconnected_fronts(&board, &mut expansions, &mut players);

    // 6. Apply troop decay
    apply_troop_decay(&mut expansions);

    // Worker will automatically dispatch at the end of this frame
}

/// Process GPU results: update board, player stats, and borders
fn process_gpu_results(
    board_out_data: &[u32],
    board: &mut Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    tile_change_writer: &mut MessageWriter<TileChangeMessage>,
    player_map: &PlayerEntityMap,
) {
    let _span = tracing::info_span!("process_gpu_results").entered();

    // Compare board_out with current board to find changes
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let index = y * BOARD_WIDTH + x;
            let new_tile_data = board_out_data[index] as u16;
            let old_tile = board.get(x, y);

            // Check if ownership changed
            let new_owner = (new_tile_data & 0x0FFF) as usize;
            let old_owner = old_tile.owner() as usize;

            if new_owner != old_owner {
                // Update the board
                board.get_mut(x, y).0 = new_tile_data;

                // Send tile change message for rendering
                tile_change_writer.write(TileChangeMessage { x, y, new_owner });

                // Update player statistics incrementally
                if let Some(new_owner_entity) = player_map.0[new_owner]
                    && let Ok((_, mut player)) = players.get_mut(new_owner_entity)
                {
                    player.tile_count += 1;
                    player.sum_x += x as u64;
                    player.sum_y += y as u64;
                }

                if old_owner != NO_OWNER
                    && let Some(old_owner_entity) = player_map.0[old_owner]
                    && let Ok((_, mut player)) = players.get_mut(old_owner_entity)
                {
                    player.tile_count -= 1;
                    player.sum_x -= x as u64;
                    player.sum_y -= y as u64;
                }

                // Update borders incrementally
                update_borders_incremental(x, y, old_owner, new_owner, board, players, player_map);
            }
        }
    }
}

/// Convert ActiveExpansions to GPU Front format
fn prepare_gpu_fronts(expansions: &ActiveExpansions) -> Vec<Front> {
    let _span = tracing::info_span!("prepare_gpu_fronts").entered();

    let mut fronts = Vec::new();

    // Iterate through all possible player pairs
    for a in 0..crate::NUM_ENTITIES {
        for b in (a + 1)..crate::NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);

            if net_troops == 0 {
                continue;
            }

            // Determine attacker and defender based on sign
            let (attacker, defender) = if net_troops > 0 { (a, b) } else { (b, a) };
            let velocity = net_troops.abs();

            // Calculate how many tiles to conquer this tick
            let tiles_to_move = (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as u32;

            fronts.push(Front {
                attacker_id: attacker as u32,
                defender_id: defender as u32,
                tiles_to_conquer: tiles_to_move,
                _padding: 0,
            });
        }
    }

    // Pad to NUM_PAIRS to match buffer size
    while fronts.len() < crate::NUM_PAIRS {
        fronts.push(Front {
            attacker_id: 0,
            defender_id: 0,
            tiles_to_conquer: 0,
            _padding: 0,
        });
    }

    fronts
}

/// Apply troop decay to all active fronts
fn apply_troop_decay(expansions: &mut ActiveExpansions) {
    let _span = tracing::info_span!("troop_decay").entered();

    for troops in expansions.fronts.iter_mut() {
        if *troops != 0 {
            let abs_troops = troops.abs();
            let decay_rate = ((abs_troops as f32 * 0.1).max(1.0) as i32).min(abs_troops);
            *troops = if *troops > 0 {
                troops.saturating_sub(decay_rate)
            } else {
                troops.saturating_add(decay_rate)
            };
        }
    }

    // Clean up empty queues (no longer needed with GPU, but keep for compatibility)
    expansions.conquer_queues.clear();
}
