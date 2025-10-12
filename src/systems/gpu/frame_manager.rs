use bevy::prelude::*;

use super::GpuPlayerStats;
use crate::NUM_ENTITIES;

#[derive(Resource, Debug)]
pub struct GpuFrameManager {
    pub tile_counts_buffers: Vec<u32>,
    pub sum_x_low_buffers: Vec<u32>,
    pub sum_x_high_buffers: Vec<u32>,
    pub sum_y_low_buffers: Vec<u32>,
    pub sum_y_high_buffers: Vec<u32>,
    pub adjacency_buffers: Vec<u32>,
}

impl GpuFrameManager {
    /// Create a new frame manager with empty buffers
    pub fn new() -> Self {
        let num_entities = NUM_ENTITIES as usize;
        // Packed adjacency: NUM_PAIRS bits packed into u32 words
        let num_pairs = crate::NUM_PAIRS as usize;
        let adjacency_size = (num_pairs + 31) / 32; // Ceiling division

        Self {
            tile_counts_buffers: vec![0u32; num_entities],
            sum_x_low_buffers: vec![0u32; num_entities],
            sum_x_high_buffers: vec![0u32; num_entities],
            sum_y_low_buffers: vec![0u32; num_entities],
            sum_y_high_buffers: vec![0u32; num_entities],
            adjacency_buffers: vec![0u32; adjacency_size],
        }
    }

    pub fn get_readable_stats(&self, i: usize) -> GpuPlayerStats {
        GpuPlayerStats {
            tile_count: self.tile_counts_buffers[i],
            sum_x_low: self.sum_x_low_buffers[i],
            sum_x_high: self.sum_x_high_buffers[i],
            sum_y_low: self.sum_y_low_buffers[i],
            sum_y_high: self.sum_y_high_buffers[i],
        }
    }

    /// Get the adjacency matrix from the readable frame (N-1 results)
    pub fn get_readable_adjacency(&self) -> &[u32] {
        &self.adjacency_buffers
    }
}

impl Default for GpuFrameManager {
    fn default() -> Self {
        Self::new()
    }
}
