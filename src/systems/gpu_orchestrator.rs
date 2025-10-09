use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;
use bytemuck::Zeroable;

use super::border_calculation::update_borders_incremental;
use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::gpu::{ExpansionWorker, GpuPlayerStats, GpuTileChange};
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::*;
use crate::{EXPANSION_RATE_BASE, NUM_ENTITIES};

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
        bevy::log::debug!("GPU still working, skipping this tick to maintain determinism");
        return;
    }

    // --- GPU has completed previous tick, read TINY results (not 16MB!) ---
    let (changed_tiles, player_stats) = {
        let _span = tracing::info_span!("load_gpu_results").entered();

        // Read the count of changed tiles
        let change_count_vec = worker.read_vec::<u32>("changed_tiles_count");
        let change_count = change_count_vec[0] as usize;

        bevy::log::info!("GPU detected {} tile changes", change_count);

        // Read ONLY the changed tiles (not the entire 16MB board!)
        let all_changes = worker.read_vec::<GpuTileChange>("changed_tiles");
        let changed_tiles = all_changes[..change_count.min(65536)].to_vec();

        // Read player statistics calculated on GPU
        let player_stats = worker.read_vec::<GpuPlayerStats>("player_stats");

        (changed_tiles, player_stats)
    };

    // Process the small list of changes
    process_gpu_results(
        &changed_tiles,
        &player_stats,
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

    // 3. Prepare GPU data: Convert active fronts to direct lookup table
    let front_lookup = prepare_front_lookup(&expansions);

    // 4. Write data to GPU buffers
    worker.write_slice("front_lookup", &front_lookup);

    // Reset atomic counters for this tick
    worker.write_slice(
        "conquest_counters",
        &vec![0u32; NUM_ENTITIES * NUM_ENTITIES],
    );

    // Reset changed_tiles_count for next tick
    worker.write_slice("changed_tiles_count", &[0u32]);

    // Reset player_stats for next tick
    worker.write_slice(
        "player_stats",
        &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES],
    );

    // 5. Clear expansion fronts for disconnected borders and refund troops
    clear_disconnected_fronts(&board, &mut expansions, &mut players);

    // 6. Apply troop decay
    apply_troop_decay(&mut expansions);

    // Worker will automatically dispatch at the end of this frame
}

/// Process GPU results: update board, player stats, and borders
/// NOW FAST: Only iterates over changed tiles, not all 4M tiles!
#[tracing::instrument(skip_all)]
fn process_gpu_results(
    changed_tiles: &[GpuTileChange],
    player_stats: &[GpuPlayerStats],
    board: &mut Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    tile_change_writer: &mut MessageWriter<TileChangeMessage>,
    player_map: &PlayerEntityMap,
) {
    // Update player statistics from GPU reduction
    {
        let _span = tracing::info_span!("update_player_stats").entered();
        for (_, mut player) in players.iter_mut() {
            let stats = &player_stats[player.id];
            player.tile_count = stats.tile_count as usize;

            let x_low = stats.sum_x_low as u64;
            let x_high = stats.sum_x_high as u64;
            player.sum_x = (x_high << 32) | x_low;

            let y_low = stats.sum_y_low as u64;
            let y_high = stats.sum_y_high as u64;
            player.sum_y = (y_high << 32) | y_low;
        }
    }

    // Process only the tiles that changed (typically hundreds, not millions)
    {
        let _span = tracing::info_span!("process_changes").entered();
        for change in changed_tiles {
            let x = change.x as usize;
            let y = change.y as usize;
            let new_owner = change.new_owner as usize;

            // Get old owner from CPU board before updating
            let old_owner = board.get(x, y).owner() as usize;

            // Update the CPU board
            board.get_mut(x, y).set_owner(new_owner as u16);

            // Send tile change message for rendering
            tile_change_writer.write(TileChangeMessage { x, y, new_owner });

            // Update borders incrementally
            update_borders_incremental(x, y, old_owner, new_owner, board, players, player_map);
        }
    }
}

/// Convert ActiveExpansions to GPU-friendly direct lookup table
/// Index = attacker * NUM_ENTITIES + defender, Value = tiles_to_conquer
#[tracing::instrument(skip_all)]
fn prepare_front_lookup(expansions: &ActiveExpansions) -> Vec<u32> {
    // Create flattened 2D array: [attacker][defender] -> tiles_to_conquer
    let mut lookup = vec![0u32; NUM_ENTITIES * NUM_ENTITIES];

    // Iterate through all possible player pairs
    for a in 0..NUM_ENTITIES {
        for b in (a + 1)..NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Determine attacker and defender based on sign
            let (attacker, defender) = if net_troops > 0 { (a, b) } else { (b, a) };
            let velocity = net_troops.abs();

            // Calculate how many tiles to conquer this tick
            let tiles_to_move = (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as u32;

            if tiles_to_move > 0 {
                // Direct lookup: O(1) access on GPU
                lookup[attacker * NUM_ENTITIES + defender] = tiles_to_move;
            }
        }
    }

    lookup
}

/// Apply troop decay to all active fronts
#[tracing::instrument(skip_all)]
fn apply_troop_decay(expansions: &mut ActiveExpansions) {
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
