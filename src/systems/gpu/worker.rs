use bevy::prelude::*;
use bevy_app_compute::prelude::*;
use bytemuck::{Pod, Zeroable};

use crate::types::Board;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, EXPANSION_RATE_BASE, NUM_ENTITIES};

/// Simulation parameters sent to the GPU as a uniform buffer
#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct SimParams {
    pub board_width: u32,
    pub board_height: u32,
    pub expansion_rate: f32,
    pub num_entities: u32, // Total number of entities (players + wilderness)
}

/// Represents a single tile change detected by the GPU
#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct GpuTileChange {
    pub x: u32,
    pub y: u32,
    pub new_owner: u32,
    pub _padding: u32,
}

/// Per-player statistics calculated on the GPU
#[derive(ShaderType, Pod, Zeroable, Clone, Copy, Debug)]
#[repr(C)]
pub struct GpuPlayerStats {
    pub tile_count: u32,
    pub _padding1: u32,
    pub sum_x_low: u32,
    pub sum_x_high: u32,
    pub sum_y_low: u32,
    pub sum_y_high: u32,
}

/// Shader definition for the expansion compute pass
#[derive(TypePath)]
struct ExpansionShader;

impl ComputeShader for ExpansionShader {
    fn shader() -> ShaderRef {
        "shaders/expansion.wgsl".into()
    }
}

/// Shader definition for the results processing pass (diffing + reduction)
#[derive(TypePath)]
struct ProcessResultsShader;

impl ComputeShader for ProcessResultsShader {
    fn shader() -> ShaderRef {
        "shaders/process_results.wgsl".into()
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
            .add_uniform(
                "params",
                &SimParams {
                    board_width: BOARD_WIDTH as u32,
                    board_height: BOARD_HEIGHT as u32,
                    expansion_rate: EXPANSION_RATE_BASE,
                    num_entities: NUM_ENTITIES as u32,
                },
            )
            // Direct lookup table: front_lookup[attacker * NUM_ENTITIES + defender] = tiles_to_conquer
            .add_storage("front_lookup", &vec![0u32; NUM_ENTITIES * NUM_ENTITIES])
            // Atomic counters using same indexing as front_lookup
            .add_storage(
                "conquest_counters",
                &vec![0u32; NUM_ENTITIES * NUM_ENTITIES],
            )
            // Ping-pong buffers for board state with staging for swap support
            .add_storage("board_in", &initial_board_data)
            .add_staging("board_in", &initial_board_data)
            .add_storage("board_out", &initial_board_data)
            .add_staging("board_out", &initial_board_data)
            // Define the expansion compute pass with 16x16 workgroup size
            .add_pass::<ExpansionShader>(
                [BOARD_WIDTH as u32 / 16, BOARD_HEIGHT as u32 / 16, 1],
                &[
                    "params",
                    "front_lookup",
                    "conquest_counters",
                    "board_in",
                    "board_out",
                ],
            )
            // Automatically swap board_in and board_out after expansion
            .add_swap("board_in", "board_out")
            // --- GPU DIFFING AND REDUCTION PASS ---
            // Buffer for tile changes (generous pre-allocation: 65K changes)
            .add_storage("changed_tiles", &vec![GpuTileChange::zeroed(); 65536])
            .add_staging("changed_tiles", &vec![GpuTileChange::zeroed(); 65536])
            // Buffer for per-player statistics
            .add_storage(
                "player_stats",
                &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES],
            )
            .add_staging(
                "player_stats",
                &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES],
            )
            // Atomic counter for number of changed tiles
            .add_storage("changed_tiles_count", &[0u32])
            .add_staging("changed_tiles_count", &[0u32])
            // Process results pass: compare board_in vs board_out, calculate stats
            .add_pass::<ProcessResultsShader>(
                [BOARD_WIDTH as u32 / 16, BOARD_HEIGHT as u32 / 16, 1],
                &[
                    "params",
                    "board_in",
                    "board_out",
                    "changed_tiles",
                    "player_stats",
                    "changed_tiles_count",
                ],
            )
            .build()
    }
}
