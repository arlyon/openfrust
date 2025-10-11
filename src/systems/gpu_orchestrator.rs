use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;
use bytemuck::Zeroable;
use std::sync::atomic::{AtomicU64, Ordering};

use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::gpu::{ExpansionWorker, GpuFrameManager, GpuPlayerStats};
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::{ActiveExpansions, Alive, PlayerData, PlayerId, PlayerInfoText};
use crate::{EXPANSION_RATE_BASE, NUM_ENTITIES};

/// Resource tracking GPU execution time in milliseconds
#[derive(Resource)]
pub struct GpuOrchestratorTime {
    time_ms: AtomicU64,
    dispatch_time: Option<f64>,
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

    pub fn mark_dispatch(&mut self, time: &Time) {
        self.dispatch_time = Some(time.elapsed_secs_f64());
    }

    pub fn mark_ready(&mut self, time: &Time) {
        if let Some(start) = self.dispatch_time.take() {
            let elapsed_s = time.elapsed_secs_f64() - start;
            self.set((elapsed_s * 1000.0) as u64);
        }
    }
}

impl Default for GpuOrchestratorTime {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU-accelerated game update system - orchestrates CPU and GPU work
///
/// This system implements a 2-frame pipeline:
/// - GPU processes Frame N while CPU prepares Frame N+1
/// - CPU uses results from Frame N-1 to make decisions
/// - This introduces 1 tick of latency but maximizes throughput
#[tracing::instrument(skip_all)]
pub fn gpu_orchestrator(
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    mut frame_manager: ResMut<GpuFrameManager>,
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
    mut timing: ResMut<GpuOrchestratorTime>,
    time: Res<Time>,
) {
    // === PIPELINE STAGE 1: Check GPU Readiness ===
    // During warmup (first 2 frames), we don't check readiness
    // After warmup, we need to ensure GPU isn't falling behind
    if frame_manager.has_valid_data() {
        if !worker.ready() {
            // GPU is more than 1 frame behind - stall to prevent overrun
            bevy::log::warn!("GPU running >1 frame behind CPU, stalling this tick");
            return;
        }

        // GPU work from previous tick is complete - record timing
        timing.mark_ready(&time);
    }

    // === PIPELINE STAGE 2: Readback GPU Results ===
    // Read results into the current write frame buffer
    // These are the results from the GPU tick we dispatched N-1 frames ago
    if worker.ready() && frame_manager.frames_dispatched > 0 {
        let _span = tracing::info_span!("load_gpu_results").entered();
        let write_frame = frame_manager.write_frame();

        // Store readback results in the write frame buffer
        frame_manager.player_stats_buffers[write_frame] =
            worker.read_vec::<GpuPlayerStats>("player_stats");
        frame_manager.adjacency_buffers[write_frame] = worker.read_vec::<u32>("adjacency_matrix");
    }

    // === PIPELINE STAGE 3: Use Data from Previous Frame ===
    // WARMUP PHASE: For the first 2 frames, we just dispatch GPU work
    // without running CPU logic, since we don't have valid results yet
    if !frame_manager.has_valid_data() {
        bevy::log::info!(
            "Pipeline warmup frame {} - dispatching GPU work",
            frame_manager.frames_dispatched
        );

        // Prepare initial GPU data
        let front_lookup = prepare_front_lookup(&expansions);
        worker.write_slice("front_lookup", &front_lookup);
        worker.write_slice(
            "conquest_counters",
            &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
        );
        worker.write_slice(
            "player_stats",
            &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES as usize],
        );
        worker.write_slice(
            "adjacency_matrix",
            &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
        );

        timing.mark_dispatch(&time);
        frame_manager.advance_frame();
        return;
    }

    // We use the read frame (opposite of write frame) which contains
    // results from the last complete GPU tick
    let player_stats = frame_manager.get_readable_stats();
    let adjacency = frame_manager.get_readable_adjacency();

    // Update player statistics from GPU (using N-1 data)
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

    // === PIPELINE STAGE 4: CPU Game Logic ===

    // 1. Check for eliminations and update troop generation
    check_eliminations_and_update_troops(&mut players, &mut expansions, &mut commands, &text_query);

    let pool = ComputeTaskPool::get();

    // 2. AI: Assign troops to expansion fronts (using adjacency from N-1)
    assign_and_log_expansions(&mut players, &mut expansions, adjacency, pool);

    // 3. Prepare GPU data: Convert active fronts to direct lookup table
    let front_lookup = prepare_front_lookup(&expansions);

    // === PIPELINE STAGE 5: Write Data for Next GPU Tick ===
    // Write data to GPU buffers for the next frame
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

    // 4. Clear expansion fronts for disconnected borders and refund troops
    clear_disconnected_fronts(&mut expansions, &mut players, adjacency);

    // 5. Apply troop decay
    apply_troop_decay(&mut expansions);

    // === PIPELINE STAGE 6: Dispatch & Advance ===
    // Worker will automatically dispatch at the end of this frame
    // Mark dispatch time to measure GPU execution
    timing.mark_dispatch(&time);

    // Advance to the next frame
    frame_manager.advance_frame();
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
