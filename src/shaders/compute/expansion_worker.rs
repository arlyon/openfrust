//! GPU pipeline design (One-Shot Mode)
//!
//! We keep 3 copies of the board. One is the "read" buffer (board_in) which is used as input to sim tick,
//! while board_out is the destination for the results of the sim tick, and board_render is kept as an
//! always-valid state for rendering.
//!
//! ## Usage
//!
//! Add the [`ExpansionPlugin`] to your app:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use openfrust::shaders::compute::expansion_worker::ExpansionPlugin;
//! App::new()
//!     .add_plugins(ExpansionPlugin)
//!     .run();
//! ```
//!
//! The plugin will automatically set up the expansion worker and the GPU read system
//! that synchronizes results back to the CPU.

use bevy::prelude::*;
use bevy_app_compute::prelude::*;
use bytemuck::{Pod, Zeroable};

use crate::map::GameMap;
use crate::shaders::{
    BorderAdjacencyShader, ClearBuffersShader, ExpansionShader, ProcessResultsShader,
};
use crate::sim_manager::SimManager;
use crate::{EXPANSION_RATE_BASE, NUM_ENTITIES, NUM_PAIRS};

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
    pub sum_x_low: u32,
    pub sum_x_high: u32,
    pub sum_y_low: u32,
    pub sum_y_high: u32,
}

/// Workgroup size configuration (must match WGSL @workgroup_size)
const WORKGROUP_SIZE_X: u32 = 16;
const WORKGROUP_SIZE_Y: u32 = 16;

/// Number of tiles each thread processes horizontally in the expansion shader
/// This is 2 because we pack two tiles into each u32
const EXPANSION_TILES_PER_THREAD_X: u32 = 2;

/// Helper function to calculate dispatch size with ceiling division
const fn div_ceil(a: u32, b: u32) -> u32 {
    (a + b - 1) / b
}

/// Compute worker for territory expansion
#[derive(Resource)]
pub struct ExpansionWorker;

impl ComputeWorker for ExpansionWorker {
    fn build(world: &mut World) -> AppComputeWorker<Self> {
        // Get map dimensions from the GameMap resource
        let map = world.resource::<GameMap>();
        let board_width = map.width();
        let board_height = map.height();

        // Pack map terrain data: 4 MapTile (u8) values per u32
        let terrain_data = map.terrain();
        let mut packed_terrain: Vec<u32> = Vec::with_capacity(terrain_data.len() / 4);
        for chunk in terrain_data.chunks(4) {
            let mut packed = 0u32;
            for (i, tile) in chunk.iter().enumerate() {
                packed |= (tile.as_byte() as u32) << (i * 8);
            }
            packed_terrain.push(packed);
        }

        let board_size_in_bytes =
            (board_width as u64 * board_height as u64) * std::mem::size_of::<u16>() as u64;

        // Calculate dispatch sizes dynamically based on workgroup configuration
        let expansion_dispatch_x = div_ceil(
            board_width as u32,
            WORKGROUP_SIZE_X * EXPANSION_TILES_PER_THREAD_X,
        );
        let expansion_dispatch_y = div_ceil(board_height as u32, WORKGROUP_SIZE_Y);

        let standard_dispatch_x = div_ceil(board_width as u32, WORKGROUP_SIZE_X);
        let standard_dispatch_y = div_ceil(board_height as u32, WORKGROUP_SIZE_Y);

        // Calculate dispatch size for the clear pass
        // We need to clear multiple buffers. We'll dispatch enough threads to cover the largest one.
        // The shader has `if` guards to prevent out-of-bounds writes.
        let conquest_len = NUM_PAIRS; // Packed: one per player pair
        let stats_len = (NUM_ENTITIES as u32) * 5; // 5 separate u32 buffers per player
        let adjacency_len = (NUM_PAIRS + 31) / 32; // Packed adjacency: NUM_PAIRS bits in u32 words
        let max_len = conquest_len.max(stats_len).max(adjacency_len);
        let clear_dispatch_x = div_ceil(max_len, 256); // 256 is workgroup_size in clear_buffers.wgsl

        // Build the compute worker with all necessary buffers
        AppComputeWorkerBuilder::new(world)
            // Uniform buffer for global simulation settings
            .add_uniform(
                "params",
                &SimParams {
                    board_width: board_width as u32,
                    board_height: board_height as u32,
                    expansion_rate: EXPANSION_RATE_BASE,
                    num_entities: u32::from(NUM_ENTITIES),
                },
            )
            // Packed front lookup: NUM_PAIRS entries with signed i32 values
            .add_storage("front_lookup", &vec![0i32; NUM_PAIRS as usize])
            // Atomic counters: NUM_PAIRS entries (one per player pair)
            .add_rw_storage("conquest_counters", &vec![0u32; NUM_PAIRS as usize])
            // Ping-pong buffers for board state with staging for swap support
            // These are created as storage assets so they can be shared with PlayerIdWorker
            .add_empty_rw_storage("board_in", board_size_in_bytes)
            .add_empty_rw_storage("board_out", board_size_in_bytes)
            // Create a read-write storage asset for rendering (synced from board_out via copy pass)
            .add_empty_rw_storage_asset("board_render", board_size_in_bytes)
            // Map terrain data - immutable, packed 4 u8 MapTiles per u32
            // Initialized with actual map data during worker build
            .add_storage_asset("map_terrain", &packed_terrain)
            // Separate buffers for per-player statistics (for workgroup reduction optimization)
            .add_storage("player_tile_counts", &vec![0u32; NUM_ENTITIES as usize])
            .add_staging("player_tile_counts", &vec![0u32; NUM_ENTITIES as usize])
            .add_storage("player_sum_x_low", &vec![0u32; NUM_ENTITIES as usize])
            .add_staging("player_sum_x_low", &vec![0u32; NUM_ENTITIES as usize])
            .add_storage("player_sum_x_high", &vec![0u32; NUM_ENTITIES as usize])
            .add_staging("player_sum_x_high", &vec![0u32; NUM_ENTITIES as usize])
            .add_storage("player_sum_y_low", &vec![0u32; NUM_ENTITIES as usize])
            .add_staging("player_sum_y_low", &vec![0u32; NUM_ENTITIES as usize])
            .add_storage("player_sum_y_high", &vec![0u32; NUM_ENTITIES as usize])
            .add_staging("player_sum_y_high", &vec![0u32; NUM_ENTITIES as usize])
            // Adjacency matrix: bit-packed for memory efficiency
            // NUM_PAIRS bits packed into u32 words (32 bits per word)
            .add_storage("adjacency_matrix", &{
                let num_words = ((NUM_PAIRS + 31) / 32) as usize; // Ceiling division
                vec![0u32; num_words]
            })
            .add_staging("adjacency_matrix", &{
                let num_words = ((NUM_PAIRS + 31) / 32) as usize; // Ceiling division
                vec![0u32; num_words]
            })
            // --- CLEAR PASS: Must run first to ensure buffers are zeroed before use ---
            // This prevents CPU-GPU race conditions by making buffer clearing part of the GPU pipeline
            .add_pass::<ClearBuffersShader>(
                [clear_dispatch_x, 1, 1],
                &[
                    "conquest_counters",
                    "player_tile_counts",
                    "player_sum_x_low",
                    "player_sum_x_high",
                    "player_sum_y_low",
                    "player_sum_y_high",
                    "adjacency_matrix",
                ],
                &[], // No storage asset buffers
            )
            .with_label("clear_buffers".into())
            // Define the expansion compute pass
            // Each thread processes 2 tiles (one packed u32)
            .add_pass::<ExpansionShader>(
                [expansion_dispatch_x, expansion_dispatch_y, 1],
                &[
                    "params",
                    "front_lookup",
                    "conquest_counters",
                    "board_in",
                    "board_out",
                ],
                &["map_terrain"], // Storage asset buffers
            )
            .with_label("expansion".into())
            // Automatically swap board_in and board_out after expansion
            .add_swap("board_in", "board_out")
            // Process results pass: calculate player stats with workgroup reduction
            .add_pass::<ProcessResultsShader>(
                [standard_dispatch_x, standard_dispatch_y, 1],
                &[
                    "params",
                    "board_out",
                    "player_tile_counts",
                    "player_sum_x_low",
                    "player_sum_x_high",
                    "player_sum_y_low",
                    "player_sum_y_high",
                ],
                &[], // No storage asset buffers
            )
            .with_label("process_results".into())
            // Border adjacency pass: determine which players border each other
            .add_pass::<BorderAdjacencyShader>(
                [standard_dispatch_x, standard_dispatch_y, 1],
                &["params", "board_out", "adjacency_matrix"],
                &[], // No storage asset buffers
            )
            .with_label("adjacency".into())
            // Copy board_in (which has now been swapped) to board_render for rendering (GPU-to-GPU)
            .add_copy(
                BufferSource::Worker("board_in"),
                BufferSource::StorageAsset("board_render"),
            )
            // Configure worker to run only when explicitly executed
            .one_shot()
            .build()
    }
}

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

/// Plugin that manages the territory expansion GPU pipeline.
///
/// This plugin sets up the complete expansion simulation pipeline:
/// - Registers the [`ExpansionWorker`] compute worker
/// - Adds the [`gpu_read`] system that synchronizes GPU results back to CPU
///
/// The [`gpu_read`] system runs in [`Update`] and reads back computation results
/// from the GPU worker when ready, updating the [`SimManager`] with the latest data.
///
/// ## Example
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use openfrust::shaders::compute::expansion_worker::ExpansionPlugin;
/// App::new()
///     .add_plugins(ExpansionPlugin)
///     .run();
/// ```
pub struct ExpansionPlugin;

impl Plugin for ExpansionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AppComputeWorkerPlugin::<ExpansionWorker>::default())
            .add_systems(Update, gpu_read);
    }
}
