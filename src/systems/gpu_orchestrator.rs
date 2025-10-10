use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;
use bytemuck::Zeroable;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::gpu::{ExpansionWorker, GpuPlayerStats};
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::{PlayerData, Alive, ActiveExpansions, PlayerInfoText, PlayerId};
use crate::{EXPANSION_RATE_BASE, NUM_ENTITIES};

/// Resource holding the adjacency matrix from the GPU
#[derive(Resource)]
pub struct AdjacencyMatrix(pub Vec<u32>);

/// Resource tracking GPU execution time in milliseconds
#[derive(Resource)]
pub struct GpuOrchestratorTime {
    time_ms: AtomicU64,
    dispatch_time: Option<Instant>,
}

impl GpuOrchestratorTime {
    pub fn new() -> Self {
        Self {
            time_ms: AtomicU64::new(0),
            dispatch_time: None,
        }
    }

    pub fn set(&self, ms: u64) {
        self.time_ms.store(ms, Ordering::Relaxed);
    }

    pub fn get(&self) -> f64 {
        self.time_ms.load(Ordering::Relaxed) as f64
    }

    pub fn mark_dispatch(&mut self) {
        self.dispatch_time = Some(Instant::now());
    }

    pub fn mark_ready(&mut self) {
        if let Some(start) = self.dispatch_time.take() {
            let elapsed_us = start.elapsed().as_micros() as u64;
            self.set((elapsed_us + 500) / 1000); // Round to nearest ms
        }
    }
}

impl Default for GpuOrchestratorTime {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU-accelerated game update system - orchestrates CPU and GPU work
#[tracing::instrument(skip_all)]
pub fn gpu_orchestrator(
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    mut adjacency: ResMut<AdjacencyMatrix>,
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
    mut timing: ResMut<GpuOrchestratorTime>,
) {
    // Synchronization point: wait for GPU to finish previous tick
    if !worker.ready() {
        bevy::log::debug!("GPU still working, skipping this tick to maintain determinism");
        return;
    }

    // GPU work from previous tick is complete - record timing
    timing.mark_ready();

    // --- GPU has completed previous tick, read TINY results ---
    let player_stats = {
        let _span = tracing::info_span!("load_gpu_results").entered();

        // Read player statistics calculated on GPU
        let player_stats = worker.read_vec::<GpuPlayerStats>("player_stats");

        // Read adjacency matrix from GPU
        adjacency.0 = worker.read_vec::<u32>("adjacency_matrix");

        player_stats
    };

    // Update player statistics from GPU
    {
        let _span = tracing::info_span!("update_player_stats").entered();
        for (_, mut player) in &mut players {
            let stats = &player_stats[usize::from(player.id)];
            player.tile_count = stats.tile_count as usize;

            let x_low = u64::from(stats.sum_x_low);
            let x_high = u64::from(stats.sum_x_high);
            player.sum_x = (x_high << 32) | x_low;

            let y_low = u64::from(stats.sum_y_low);
            let y_high = u64::from(stats.sum_y_high);
            player.sum_y = (y_high << 32) | y_low;
        }
    }

    // --- CPU Phase: High-level game logic ---

    // 1. Check for eliminations and update troop generation
    check_eliminations_and_update_troops(&mut players, &mut expansions, &mut commands, &text_query);

    let pool = ComputeTaskPool::get();

    // 2. AI: Assign troops to expansion fronts
    assign_and_log_expansions(&mut players, &mut expansions, &adjacency, pool);

    // 3. Prepare GPU data: Convert active fronts to direct lookup table
    let front_lookup = prepare_front_lookup(&expansions);

    // 4. Write data to GPU buffers
    worker.write_slice("front_lookup", &front_lookup);

    // Reset atomic counters for this tick
    worker.write_slice(
        "conquest_counters",
        &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
    );

    // Reset player_stats for next tick
    worker.write_slice(
        "player_stats",
        &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES as usize],
    );

    // Reset adjacency_matrix for next tick
    worker.write_slice(
        "adjacency_matrix",
        &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
    );

    // 5. Clear expansion fronts for disconnected borders and refund troops
    clear_disconnected_fronts(&mut expansions, &mut players, &adjacency);

    // 6. Apply troop decay
    apply_troop_decay(&mut expansions);

    // Worker will automatically dispatch at the end of this frame
    // Mark dispatch time to measure GPU execution
    timing.mark_dispatch();
}

/// Convert `ActiveExpansions` to GPU-friendly direct lookup table
/// Index = attacker * `NUM_ENTITIES` + defender, Value = `tiles_to_conquer`
#[tracing::instrument(skip_all)]
fn prepare_front_lookup(expansions: &ActiveExpansions) -> Vec<u32> {
    // Create flattened 2D array: [attacker][defender] -> tiles_to_conquer
    let mut lookup = vec![0; (NUM_ENTITIES * NUM_ENTITIES) as usize];

    // Iterate through all possible player pairs
    for a in 0..NUM_ENTITIES {
        for b in (a + 1)..NUM_ENTITIES {
            let net_troops =
                expansions.get_net_troops(PlayerId::new_unchecked(a), PlayerId::new_unchecked(b));
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
                lookup[(attacker * NUM_ENTITIES + defender) as usize] = tiles_to_move;
            }
        }
    }

    lookup
}

/// Apply troop decay to all active fronts
#[tracing::instrument(skip_all)]
fn apply_troop_decay(expansions: &mut ActiveExpansions) {
    for troops in &mut expansions.fronts {
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
