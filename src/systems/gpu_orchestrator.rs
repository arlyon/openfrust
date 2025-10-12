use bevy::prelude::*;
use bevy_app_compute::prelude::*;

use super::gpu::ExpansionWorker;
use super::sim_manager::SimManager;

/// Bevy system that acts as a thin adapter to drive the SimManager.
///
/// This system runs in FixedUpdate at 10Hz and coordinates the entire simulation.
/// All the complex logic lives in SimManager - this is just the ECS integration layer.
///
/// Responsibilities:
/// - Gather data from ECS (queries, resources)
/// - Pass data to SimManager.tick()
/// - Handle status reporting (logging)
#[tracing::instrument(skip_all)]
pub fn gpu_read(
    // The manager that now holds all the state and logic
    mut sim_manager: ResMut<SimManager>,
    // Dependencies required by the manager's tick method
    worker: Res<AppComputeWorker<ExpansionWorker>>,
    time: Res<Time>,
) {
    if worker.ready() {
        tracing::debug!("reading: {:?} {:?}", worker.state, worker.run_mode);
        // Read back all GPU computation results into the write frame buffer
        sim_manager.frame_manager.tile_counts_buffers =
            worker.read_vec::<u32>("player_tile_counts");
        sim_manager.frame_manager.sum_x_low_buffers = worker.read_vec::<u32>("player_sum_x_low");
        sim_manager.frame_manager.sum_x_high_buffers = worker.read_vec::<u32>("player_sum_x_high");
        sim_manager.frame_manager.sum_y_low_buffers = worker.read_vec::<u32>("player_sum_y_low");
        sim_manager.frame_manager.sum_y_high_buffers = worker.read_vec::<u32>("player_sum_y_high");
        sim_manager.frame_manager.adjacency_buffers = worker.read_vec::<u32>("adjacency_matrix");
        sim_manager.timing.mark_ready(&time);
    }
}
