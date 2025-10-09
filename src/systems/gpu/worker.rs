use bevy::prelude::*;
use bevy_app_compute::prelude::*;
use bytemuck::{Pod, Zeroable};

use crate::types::Board;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, EXPANSION_RATE_BASE, NUM_PAIRS};

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

/// Shader definition for the expansion compute pass
#[derive(TypePath)]
struct ExpansionShader;

impl ComputeShader for ExpansionShader {
    fn shader() -> ShaderRef {
        "shaders/expansion.wgsl".into()
    }
}

/// Compute worker for territory expansion
#[derive(Resource)]
pub struct ExpansionWorker;

impl ComputeWorker for ExpansionWorker {
    fn build(world: &mut World) -> AppComputeWorker<Self> {
        // Get initial board state from the world
        let board = world.resource::<Board>();

        // Convert Tile(u16) to u32 for WGSL compatibility (WGSL doesn't have u16 in storage buffers)
        let initial_board_data: Vec<u32> = board.tiles.iter().map(|t| t.0 as u32).collect();

        // Build the compute worker with all necessary buffers
        AppComputeWorkerBuilder::new(world)
            // Uniform buffer for global simulation settings
            .add_uniform("params", &SimParams {
                board_width: BOARD_WIDTH as u32,
                board_height: BOARD_HEIGHT as u32,
                expansion_rate: EXPANSION_RATE_BASE,
                _padding: 0,
            })

            // Storage buffer for the list of active battlefronts
            .add_storage("fronts", &vec![Front::zeroed(); NUM_PAIRS])

            // Storage buffer for atomic counters (one per front)
            .add_storage("conquer_counters", &vec![0u32; NUM_PAIRS])

            // Ping-pong buffers for board state
            .add_storage("board_in", &initial_board_data)
            .add_storage("board_out", &initial_board_data)

            // Define the compute pass with 16x16 workgroup size
            .add_pass::<ExpansionShader>(
                [BOARD_WIDTH as u32 / 16, BOARD_HEIGHT as u32 / 16, 1],
                &["params", "fronts", "conquer_counters", "board_in", "board_out"],
            )

            // Automatically swap board_in and board_out after each dispatch
            .add_swap("board_in", "board_out")

            .build()
    }
}
