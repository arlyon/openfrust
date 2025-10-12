use bevy::prelude::*;

use super::GpuPlayerStats;
use crate::NUM_ENTITIES;

/// Manages the GPU pipeline with double-buffered result storage.
///
/// The double buffering serves two purposes:
/// 1. Prevents race conditions when reading/writing GPU results
/// 2. Allows using previous frame's data while current frame writes new results
///
/// Timeline with one_shot() mode:
/// - Tick 0: Execute GPU, frames_dispatched = 1 (no data yet)
/// - Tick 1: Read tick 0 results into buffer 0, has_valid_data() = true
/// - Tick 2+: Read into buffer (N%2), use buffer ((N-1)%2) for CPU logic
///
/// Note: Only requires 1 warmup tick since we explicitly wait for worker.ready()
#[derive(Resource, Debug)]
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
        // Packed adjacency: NUM_PAIRS bits packed into u32 words
        let num_pairs = crate::NUM_PAIRS as usize;
        let adjacency_size = (num_pairs + 31) / 32; // Ceiling division

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
        0
    }

    /// Get the frame index we should WRITE results into (current tick's GPU results)
    pub fn write_frame(&self) -> usize {
        // Write to the opposite frame
        0
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

        tracing::info!("GpuFrameManager::get_readable_stats(): {:?}", stats);

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
        // With one_shot() mode, we only need 1 warmup tick:
        // - Tick 0: Execute GPU work (no data to read yet)
        // - Tick 1: Read results, now have valid data!
        //
        // We used to require >= 2 for the old automatic mode where GPU work
        // could overlap unpredictably. Now with explicit worker.ready() checks,
        // we know tick 0 is complete before tick 1 reads it.
        self.frames_dispatched >= 1
    }
}

impl Default for GpuFrameManager {
    fn default() -> Self {
        Self::new()
    }
}
