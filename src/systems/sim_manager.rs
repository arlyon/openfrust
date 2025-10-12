use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use super::disconnected_fronts::clear_disconnected_fronts;
use super::expansion_assignment::assign_and_log_expansions;
use super::gpu::{ExpansionWorker, GpuFrameManager};
use super::player_elimination::check_eliminations_and_update_troops;
use crate::types::{
    ActiveExpansions, Alive, PlayerData, PlayerEntityMap, PlayerId, PlayerInfoText,
};
use crate::{EXPANSION_RATE_BASE, NUM_ENTITIES};

/// Status returned from a simulation tick
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimStatus {
    /// GPU is still processing previous tick, this tick was skipped
    Stalled,
    /// Pipeline is warming up (first 2 ticks), CPU logic not yet running
    WarmingUp,
    /// Tick completed successfully with full CPU logic
    TickComplete,
}

/// Resource tracking GPU execution time in milliseconds
#[derive(Resource, Debug)]
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

/// Manages the entire simulation state and lifecycle.
///
/// This struct owns the GPU pipeline state and exposes a domain-friendly API.
/// The main entry point is `tick()`, which executes one complete simulation step.
///
/// Design:
/// - Owns GpuFrameManager (double-buffered pipeline state)
/// - Owns GpuOrchestratorTime (profiling/timing state)
/// - Coordinates GPU and CPU work through explicit stages
/// - Returns SimStatus to indicate what happened during the tick
#[derive(Resource, Debug)]
pub struct SimManager {
    frame_manager: GpuFrameManager,
    timing: GpuOrchestratorTime,
}

impl Default for SimManager {
    fn default() -> Self {
        Self {
            frame_manager: GpuFrameManager::default(),
            timing: GpuOrchestratorTime::default(),
        }
    }
}

impl SimManager {
    /// Executes a single simulation tick.
    ///
    /// This is the main entry point for the simulation logic. It coordinates:
    /// 1. Waiting for previous GPU work to complete
    /// 2. Reading back GPU results
    /// 3. Running CPU game logic (after warmup)
    /// 4. Preparing and dispatching next GPU workload
    /// 5. Advancing the frame counter
    ///
    /// Returns SimStatus indicating the outcome of this tick.
    #[tracing::instrument(skip_all)]
    pub fn tick(
        &mut self,
        // GPU interface
        worker: &mut AppComputeWorker<ExpansionWorker>,
        // ECS data (passed from the calling system)
        players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
        expansions: &mut ActiveExpansions,
        commands: &mut Commands,
        text_query: &Query<(Entity, &PlayerInfoText)>,
        player_map: &PlayerEntityMap,
        // Bevy resources
        time: &Time,
    ) -> SimStatus {
        bevy::log::info!("SimManager::tick()");

        // === STAGE 1: Wait for Previous GPU Work ===
        // With one_shot(), worker.ready() tells us if the last execute() completed
        if self.frame_manager.frames_dispatched > 0 && !worker.ready() {
            bevy::log::warn!(
                "GPU work from tick {} still running, stalling tick {} {:?}",
                self.frame_manager.frames_dispatched - 1,
                self.frame_manager.frames_dispatched,
                worker.run_mode
            );

            if worker.ready_to_execute() {
                bevy::log::info!("Executing GPU workload");
                worker.execute();
            }

            return SimStatus::Stalled;
        }

        // === STAGE 2: Read GPU Results (if available) ===
        // After first tick completes, we can read back results
        if self.frame_manager.frames_dispatched > 0 {
            self.timing.mark_ready(time);
            self.read_gpu_results(worker);
        }

        // === STAGE 3: Process CPU Logic (skip during warmup) ===
        // We need 2 completed ticks before we have valid double-buffered data
        let status = if !self.frame_manager.has_valid_data() {
            bevy::log::info!(
                "Pipeline warmup: tick {} - skipping CPU logic",
                self.frame_manager.frames_dispatched
            );
            SimStatus::WarmingUp
        } else {
            self.run_cpu_logic(players, expansions, commands, text_query, player_map);
            SimStatus::TickComplete
        };

        // === STAGE 4: Prepare and Execute Next GPU Tick ===
        self.prepare_and_dispatch_gpu_tick(expansions, worker, time);

        // === STAGE 5: Advance Frame Counter ===
        self.frame_manager.advance_frame();

        status
    }

    /// Borrows the timing data for external use (e.g., Perf UI).
    pub fn timing(&self) -> &GpuOrchestratorTime {
        &self.timing
    }

    /// Borrows the frame manager for debugging/inspection.
    #[allow(dead_code)]
    pub fn frame_manager(&self) -> &GpuFrameManager {
        &self.frame_manager
    }

    /// Helper to encapsulate reading data from the GPU worker.
    fn read_gpu_results(&mut self, worker: &mut AppComputeWorker<ExpansionWorker>) {
        let _span = tracing::info_span!("read_gpu_results").entered();
        let write_frame = self.frame_manager.write_frame();

        // Read back all GPU computation results into the write frame buffer
        self.frame_manager.tile_counts_buffers[write_frame] =
            worker.read_vec::<u32>("player_tile_counts");
        self.frame_manager.sum_x_low_buffers[write_frame] =
            worker.read_vec::<u32>("player_sum_x_low");
        self.frame_manager.sum_x_high_buffers[write_frame] =
            worker.read_vec::<u32>("player_sum_x_high");
        self.frame_manager.sum_y_low_buffers[write_frame] =
            worker.read_vec::<u32>("player_sum_y_low");
        self.frame_manager.sum_y_high_buffers[write_frame] =
            worker.read_vec::<u32>("player_sum_y_high");
        self.frame_manager.adjacency_buffers[write_frame] =
            worker.read_vec::<u32>("adjacency_matrix");

        bevy::log::info!("Read GPU results {:?}", self);
    }

    /// Helper to encapsulate all CPU-side game logic for a tick.
    ///
    /// This runs after GPU results are available and includes:
    /// - Updating ECS components from GPU results
    /// - Checking for player eliminations
    /// - AI troop assignment
    /// - Clearing disconnected fronts
    /// - Applying troop decay
    fn run_cpu_logic(
        &self,
        players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
        expansions: &mut ActiveExpansions,
        commands: &mut Commands,
        text_query: &Query<(Entity, &PlayerInfoText)>,
        player_map: &PlayerEntityMap,
    ) {
        let _span = tracing::info_span!("run_cpu_logic").entered();

        // Get readable data from the frame that's NOT being written to
        let player_stats = self.frame_manager.get_readable_stats();
        let adjacency = self.frame_manager.get_readable_adjacency();

        // Update ECS components from GPU results
        {
            let _span = tracing::info_span!("update_player_stats").entered();
            for (_, mut player) in players.iter_mut() {
                let stats = &player_stats[usize::from(player.id)];
                player.tile_count = stats.tile_count as usize;
                player.sum_x = (u64::from(stats.sum_x_high) << 32) | u64::from(stats.sum_x_low);
                player.sum_y = (u64::from(stats.sum_y_high) << 32) | u64::from(stats.sum_y_low);
                tracing::info!("Player {} stats: {:?}", player.id, player);
            }
        }

        // Run game logic systems
        check_eliminations_and_update_troops(players, expansions, commands, text_query);
        assign_and_log_expansions(players, expansions, adjacency, ComputeTaskPool::get());
        clear_disconnected_fronts(expansions, players, adjacency, player_map);
        apply_troop_decay(expansions);
    }

    /// Helper to prepare data and dispatch the compute worker.
    fn prepare_and_dispatch_gpu_tick(
        &mut self,
        expansions: &ActiveExpansions,
        worker: &mut AppComputeWorker<ExpansionWorker>,
        time: &Time,
    ) {
        let _span = tracing::info_span!("prepare_and_dispatch_gpu_tick").entered();

        // Convert active expansion fronts to GPU-friendly format
        let front_lookup = prepare_front_lookup(expansions);
        worker.write_slice("front_lookup", &front_lookup);

        // Mark dispatch time and execute GPU workload
        self.timing.mark_dispatch(time);
        bevy::log::info!("Executing GPU workload");
        worker.execute();
    }
}

/// Convert `ActiveExpansions` to GPU-friendly packed lookup table.
///
/// Uses triangular packing: positive = a->b, negative = b->a
fn prepare_front_lookup(expansions: &ActiveExpansions) -> Vec<i32> {
    let _span = tracing::info_span!("prepare_front_lookup").entered();

    // Create packed array: NUM_PAIRS entries with signed values
    let mut lookup = vec![0i32; crate::NUM_PAIRS as usize];

    // Iterate through all possible player pairs
    for a in 0..NUM_ENTITIES {
        for b in (a + 1)..NUM_ENTITIES {
            let a_id = PlayerId::new_unchecked(a);
            let b_id = PlayerId::new_unchecked(b);
            let net_troops = expansions.get_net_troops(a_id, b_id);

            if net_troops == 0 {
                continue;
            }

            // Calculate how many tiles to conquer this tick
            let velocity = net_troops.abs();
            let tiles_to_move =
                ((velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as i32).min(i32::MAX);

            if tiles_to_move > 0 {
                // Store signed value: positive = a->b, negative = b->a
                let signed_tiles = if net_troops > 0 {
                    tiles_to_move
                } else {
                    -tiles_to_move
                };

                // Use the same pair_index formula as ActiveExpansions
                let idx = ActiveExpansions::pair_index(a_id, b_id);
                lookup[idx] = signed_tiles;
            }
        }
    }

    lookup
}

/// Apply troop decay to all active fronts.
fn apply_troop_decay(expansions: &mut ActiveExpansions) {
    let _span = tracing::info_span!("apply_troop_decay").entered();

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
}
