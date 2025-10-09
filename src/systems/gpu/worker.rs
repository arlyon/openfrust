use bevy::prelude::*;
use bevy_app_compute::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Simulation parameters sent to the GPU as a uniform buffer
#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct SimParams {
    pub board_width: u32,
    pub board_height: u32,
    pub expansion_rate: f32,
    pub _padding: u32, // Ensures std140 alignment
}

/// Represents a single battlefront command for the GPU
#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct Front {
    pub attacker_id: u32,
    pub defender_id: u32,
    pub tiles_to_conquer: u32,
    pub _padding: u32, // Ensures std140 alignment
}

/// Compute worker for territory expansion
#[derive(Resource)]
pub struct ExpansionWorker;

// Worker implementation will be added in Phase 2
