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

/// Shader definition for the results processing pass (reduction only)
#[derive(TypePath)]
struct ProcessResultsShader;

impl ComputeShader for ProcessResultsShader {
    fn shader() -> ShaderRef {
        "shaders/process_results.wgsl".into()
    }
}

/// Shader definition for the border adjacency calculation pass
#[derive(TypePath)]
struct BorderAdjacencyShader;

impl ComputeShader for BorderAdjacencyShader {
    fn shader() -> ShaderRef {
        "shaders/border_adjacency.wgsl".into()
    }
}

/// Shader definition for copying `board_out` to `board_render` for rendering
#[derive(TypePath)]
struct CopyBoardShader;

impl ComputeShader for CopyBoardShader {
    fn shader() -> ShaderRef {
        "shaders/copy_board.wgsl".into()
    }
}

/// Compute worker for territory expansion
#[derive(Resource)]
pub struct ExpansionWorker;

impl ComputeWorker for ExpansionWorker {
    fn build(world: &mut World) -> AppComputeWorker<Self> {
        // Get initial board state from the world
        let board = world.resource::<Board>();

        // Pack two Tile(u16) values into each u32 for better memory efficiency
        // Each u32 contains: [tile2 (upper 16 bits) | tile1 (lower 16 bits)]
        let initial_board_data: Vec<u32> = board
            .tiles
            .chunks_exact(2)
            .map(|chunk| {
                let tile1 = u32::from(chunk[0].0); // Lower 16 bits
                let tile2 = u32::from(chunk[1].0); // Upper 16 bits
                (tile2 << 16) | tile1
            })
            .collect();

        // Build the compute worker with all necessary buffers
        AppComputeWorkerBuilder::new(world)
            // Uniform buffer for global simulation settings
            .add_uniform(
                "params",
                &SimParams {
                    board_width: BOARD_WIDTH as u32,
                    board_height: BOARD_HEIGHT as u32,
                    expansion_rate: EXPANSION_RATE_BASE,
                    num_entities: u32::from(NUM_ENTITIES),
                },
            )
            // Direct lookup table: front_lookup[attacker * NUM_ENTITIES + defender] = tiles_to_conquer
            .add_storage(
                "front_lookup",
                &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
            )
            // Atomic counters using same indexing as front_lookup
            .add_storage(
                "conquest_counters",
                &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
            )
            // Ping-pong buffers for board state with staging for swap support
            .add_storage("board_in", &initial_board_data)
            .add_staging("board_in", &initial_board_data)
            .add_storage("board_out", &initial_board_data)
            .add_staging("board_out", &initial_board_data)
            // Create a read-write storage asset for rendering (synced from board_out via copy pass)
            .add_rw_storage_asset("board_render", &initial_board_data)
            // Define the expansion compute pass with 16x16 workgroup size
            // Each thread processes 2 tiles (one packed u32), so dispatch width is halved
            .add_pass::<ExpansionShader>(
                [BOARD_WIDTH as u32 / 32, BOARD_HEIGHT as u32 / 16, 1],
                &[
                    "params",
                    "front_lookup",
                    "conquest_counters",
                    "board_in",
                    "board_out",
                ],
                &[],
            )
            // Automatically swap board_in and board_out after expansion
            .add_swap("board_in", "board_out")
            // --- GPU REDUCTION PASS ---
            // Buffer for per-player statistics
            .add_storage(
                "player_stats",
                &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES as usize],
            )
            .add_staging(
                "player_stats",
                &vec![GpuPlayerStats::zeroed(); NUM_ENTITIES as usize],
            )
            // Process results pass: calculate player stats (no diffing)
            .add_pass::<ProcessResultsShader>(
                [BOARD_WIDTH as u32 / 16, BOARD_HEIGHT as u32 / 16, 1],
                &["params", "board_out", "player_stats"],
                &[],
            )
            // --- BORDER ADJACENCY PASS ---
            // Adjacency matrix: [player_a * NUM_ENTITIES + player_b] = 1 if adjacent, 0 otherwise
            .add_storage(
                "adjacency_matrix",
                &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
            )
            .add_staging(
                "adjacency_matrix",
                &vec![0u32; (NUM_ENTITIES * NUM_ENTITIES) as usize],
            )
            // Border adjacency pass: determine which players border each other
            .add_pass::<BorderAdjacencyShader>(
                [BOARD_WIDTH as u32 / 16, BOARD_HEIGHT as u32 / 16, 1],
                &["params", "board_out", "adjacency_matrix"],
                &[],
            )
            // Copy board_out to board_render for rendering (GPU-to-GPU)
            // Data is packed, so width dispatch is halved
            .add_pass::<CopyBoardShader>(
                [BOARD_WIDTH as u32 / 32, BOARD_HEIGHT as u32 / 16, 1],
                &["params", "board_out"],
                &["board_render"],
            )
            .build()
    }
}
