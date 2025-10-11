use bevy::prelude::*;

use super::GpuPlayerStats;
use crate::{ADJACENCY_MATRIX_SIZE, NUM_ENTITIES};

/// Manages the asynchronous GPU pipeline with 2 frames in flight.
///
/// This enables the CPU and GPU to work in parallel:
/// - While GPU processes Frame N, CPU prepares Frame N+1 using results from Frame N-1
/// - Introduces 1 tick of logical latency (acceptable for this simulation)
/// - Dramatically improves throughput by eliminating CPU idle time
#[derive(Resource)]
pub struct GpuFrameManager {
    /// Which frame we're currently preparing to dispatch (alternates 0/1)
    pub current_frame: usize,

    /// Double-buffered CPU-side copies of GPU readback results
    /// Separate buffers for each statistic (optimized for workgroup reduction)
    pub tile_counts_buffers: [Vec<u32>; 2],
    pub sum_x_low_buffers: [Vec<u32>; 2],
    pub sum_x_high_buffers: [Vec<u32>; 2],
    pub sum_y_low_buffers: [Vec<u32>; 2],
    pub sum_y_high_buffers: [Vec<u32>; 2],

    /// Double-buffered adjacency matrix results
    /// adjacency_buffers[0] holds results from frame 0
    /// adjacency_buffers[1] holds results from frame 1
    pub adjacency_buffers: [Vec<u32>; 2],

    /// Total number of frames dispatched (used for pipeline warmup)
    pub frames_dispatched: usize,
}

impl GpuFrameManager {
    /// Create a new frame manager with empty buffers
    pub fn new() -> Self {
        let num_entities = NUM_ENTITIES as usize;
        let adjacency_size = ADJACENCY_MATRIX_SIZE as usize;

        Self {
            current_frame: 0,
            tile_counts_buffers: [vec![0u32; num_entities], vec![0u32; num_entities]],
            sum_x_low_buffers: [vec![0u32; num_entities], vec![0u32; num_entities]],
            sum_x_high_buffers: [vec![0u32; num_entities], vec![0u32; num_entities]],
            sum_y_low_buffers: [vec![0u32; num_entities], vec![0u32; num_entities]],
            sum_y_high_buffers: [vec![0u32; num_entities], vec![0u32; num_entities]],
            adjacency_buffers: [vec![0u32; adjacency_size], vec![0u32; adjacency_size]],
            frames_dispatched: 0,
        }
    }

    /// Get the frame index we should READ from (results from previous tick)
    pub fn read_frame(&self) -> usize {
        // We read from the opposite frame we're writing to
        (self.current_frame + 1) % 2
    }

    /// Get the frame index we should WRITE results into (current tick's GPU results)
    pub fn write_frame(&self) -> usize {
        self.current_frame
    }

    /// Get the player stats from the readable frame (N-1 results)
    /// Returns reconstructed GpuPlayerStats from separate buffers
    pub fn get_readable_stats(&self) -> Vec<GpuPlayerStats> {
        let frame = self.read_frame();
        let num_entities = NUM_ENTITIES as usize;

        let mut stats = Vec::with_capacity(num_entities);
        for i in 0..num_entities {
            stats.push(GpuPlayerStats {
                tile_count: self.tile_counts_buffers[frame][i],
                sum_x_low: self.sum_x_low_buffers[frame][i],
                sum_x_high: self.sum_x_high_buffers[frame][i],
                sum_y_low: self.sum_y_low_buffers[frame][i],
                sum_y_high: self.sum_y_high_buffers[frame][i],
            });
        }
        stats
    }

    /// Get the adjacency matrix from the readable frame (N-1 results)
    pub fn get_readable_adjacency(&self) -> &[u32] {
        &self.adjacency_buffers[self.read_frame()]
    }

    /// Advance to the next frame
    pub fn advance_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % 2;
        self.frames_dispatched += 1;
    }

    /// Check if the pipeline has valid data to read (warmup complete)
    pub fn has_valid_data(&self) -> bool {
        // We need at least 2 frames dispatched before we have valid results
        self.frames_dispatched >= 2
    }
}

impl Default for GpuFrameManager {
    fn default() -> Self {
        Self::new()
    }
}
