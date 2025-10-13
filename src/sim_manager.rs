use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use bevy_app_compute::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::shaders::compute::ExpansionWorker;

use crate::shaders::GpuFrameManager;
use crate::systems::{ai, check_eliminations_and_update_troops, clear_disconnected_fronts};
use crate::types::{ActiveExpansions, Alive, PlayerData, PlayerEntityMap, PlayerInfoText};

/// Status returned from a simulation tick
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimStatus {
    /// GPU is still processing previous tick, this tick was skipped
    Stalled,
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

    /// mark the time when the GPU has dispatched a workload
    pub fn mark_dispatch(&mut self, time: &Time) {
        self.dispatch_time = Some(time.elapsed_secs_f64());
    }

    /// mark the time when the GPU has results
    pub fn mark_ready(&mut self, time: &Time) {
        if let Some(start) = self.dispatch_time.take() {
            let elapsed_s = time.elapsed_secs_f64() - start;
            self.set((elapsed_s * 1000.0) as u64);
            self.dispatch_time = None;
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
#[derive(Resource, Debug)]
pub struct SimManager {
    pub frame_manager: GpuFrameManager,
    pub timing: GpuOrchestratorTime,
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
    /// # Returns
    /// SimStatus indicating the outcome of this tick.
    ///
    /// TODO HOW DO WE ENSURE:
    /// - GPU IS READY TO BE WRITTEN TO
    /// - CPU HAS RECENT DATA
    #[tracing::instrument(skip_all)]
    pub fn tick(
        &mut self,
        players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
        expansions: &mut ActiveExpansions,
        commands: &mut Commands,
        worker: &mut AppComputeWorker<ExpansionWorker>,
        text_query: &Query<(Entity, &PlayerInfoText)>,
        player_map: &PlayerEntityMap,
        time: &Time,
    ) -> SimStatus {
        if worker.state.is_created() {
            self.dispatch_gpu_tick(expansions, worker, time);
            return SimStatus::TickComplete;
        }

        self.run_cpu_logic(players, expansions, commands, text_query, player_map);
        self.dispatch_gpu_tick(expansions, worker, time);

        SimStatus::TickComplete
    }

    /// Borrows the timing data for external use (e.g., Perf UI).
    pub fn timing(&self) -> &GpuOrchestratorTime {
        &self.timing
    }

    /// Helper to prepare data and dispatch the compute worker.
    #[tracing::instrument(skip_all)]
    fn dispatch_gpu_tick(
        &mut self,
        expansions: &ActiveExpansions,
        worker: &mut AppComputeWorker<ExpansionWorker>,
        time: &Time,
    ) {
        tracing::trace!(
            "executing gpu workload {:?} {:?}",
            worker.state,
            worker.run_mode
        );

        worker
            .try_write_slice("front_lookup", &expansions.front_lookup)
            .expect("OK");

        worker.execute();
        self.timing.mark_dispatch(time);
    }

    /// Helper to encapsulate all CPU-side game logic for a tick.
    ///
    /// This runs after GPU results are available and includes:
    /// - Updating ECS components from GPU results
    /// - Checking for player eliminations
    /// - AI troop assignment
    /// - Clearing disconnected fronts
    /// - Applying troop decay
    #[tracing::instrument(skip_all)]
    fn run_cpu_logic(
        &self,
        players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
        expansions: &mut ActiveExpansions,
        commands: &mut Commands,
        text_query: &Query<(Entity, &PlayerInfoText)>,
        player_map: &PlayerEntityMap,
    ) {
        let adjacency = self.frame_manager.get_readable_adjacency();

        // Update ECS components from GPU results
        {
            let _span = tracing::info_span!("update_player_stats").entered();
            for (_, mut player) in players.iter_mut() {
                let stats = &self.frame_manager.get_readable_stats(player.id.into());
                player.tile_count = stats.tile_count as usize;
                player.sum_x = (u64::from(stats.sum_x_high) << 32) | u64::from(stats.sum_x_low);
                player.sum_y = (u64::from(stats.sum_y_high) << 32) | u64::from(stats.sum_y_low);
                tracing::debug!("Player {} stats: {:?}", player.id, player);
            }
        }

        // Run game logic systems
        check_eliminations_and_update_troops(players, expansions, commands, text_query);
        ai::assign_and_log_expansions(players, expansions, adjacency, ComputeTaskPool::get());
        clear_disconnected_fronts(expansions, players, adjacency, player_map);
        apply_troop_decay(expansions);
    }
}

/// Apply troop decay to all active fronts.
#[tracing::instrument(skip_all)]
fn apply_troop_decay(expansions: &mut ActiveExpansions) {
    for troops in &mut expansions.front_lookup {
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
