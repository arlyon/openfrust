//! Cursor-based player ID query system using GPU compute
//!
//! This module provides an asynchronous workflow for querying which player
//! owns the tile under the cursor. It uses a separate one_shot ComputeWorker
//! that shares the board_in buffer with the main ExpansionWorker.
//!
//! ## Usage
//!
//! Add the [`CursorQueryPlugin`] to your app:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use openfrust::shaders::compute::cursor_worker::CursorQueryPlugin;
//! App::new()
//!     .add_plugins(CursorQueryPlugin)
//!     .run();
//! ```
//!
//! The plugin will automatically track the cursor position and query the player ID
//! for the tile under the cursor. Read the result from the [`CursorIDResult`] resource:
//!
//! ```no_run
//! # use bevy::prelude::*;
//! # use openfrust::shaders::compute::cursor_worker::CursorIDResult;
//! fn display_player_info(cursor_result: Res<CursorIDResult>) {
//!     if let Some(player_id) = cursor_result.player_id {
//!         println!("Cursor is over player {}", player_id);
//!     }
//! }
//! ```

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_app_compute::prelude::*;

use crate::map::GameMap;
use crate::shaders::compute::ExpansionWorker;

/// Compute worker for querying player ID at a specific coordinate
#[derive(Resource)]
pub struct PlayerIdWorker;

impl ComputeWorker for PlayerIdWorker {
    fn build(world: &mut World) -> AppComputeWorker<Self> {
        // Get the main ExpansionWorker to access the shared board_in buffer
        let expansion_worker = world.resource::<AppComputeWorker<ExpansionWorker>>();

        // Get the handle to the shared board_in buffer
        let board_handle = expansion_worker
            .get_storage_buffer_asset_handle("board_render")
            .expect("Failed to get handle for 'board_render' from ExpansionWorker")
            .clone();

        // Get map dimensions
        let map = world.resource::<GameMap>();
        let board_width = map.width() as u32;
        let board_height = map.height() as u32;

        // Build the worker with shared buffer access
        AppComputeWorkerBuilder::new(world)
            // Board dimensions uniform
            .add_uniform("board_dims", &UVec2::new(board_width, board_height))
            // Target coordinate to query (will be updated by system)
            .add_uniform("target_coord", &UVec2::ZERO)
            // Shared board buffer (read-only for this worker)
            .add_storage_asset_by_handle("board_data", board_handle)
            // Result buffer
            .add_rw_storage("result_id", &[0xFFFFFFFFu32; 1])
            .add_staging("result_id", &[0xFFFFFFFFu32; 1])
            // Define the compute pass
            .add_pass::<crate::shaders::GetPlayerIdShader>(
                [1, 1, 1], // Single thread for a single lookup
                &["board_dims", "target_coord", "result_id"],
                &["board_data"],
            )
            .with_label("get_player_id".into())
            // One-shot mode: only runs when explicitly executed
            .one_shot()
            .build()
    }
}

/// Resource to hold the current query request state.
/// Updated by a system that tracks the cursor position.
#[derive(Resource, Default, Debug)]
pub struct CursorIDQuery {
    /// The integer board coordinate we want to query
    pub board_coord: Option<UVec2>,
}

/// Resource to hold the final result from the GPU.
/// The UI can read from this every frame without worrying about GPU state.
#[derive(Resource, Default, Debug)]
pub struct CursorIDResult {
    pub player_id: Option<u32>,
}

/// System 1: Track cursor and convert its position to a board coordinate
pub fn update_cursor_query(
    mut cursor_query: ResMut<CursorIDQuery>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    map: Res<GameMap>,
) {
    let Some(primary_window) = window_query.iter().next() else {
        return;
    };
    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        return;
    };

    if let Some(screen_pos) = primary_window.cursor_position() {
        // Convert screen position to world position
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) {
            // Convert world position to board coordinates (0,0 is top-left)
            let half_width = map.width() as f32 / 2.0;
            let half_height = map.height() as f32 / 2.0;

            let x = (world_pos.x + half_width).floor();
            let y = (half_height - world_pos.y).floor();

            if x >= 0.0 && x < map.width() as f32 && y >= 0.0 && y < map.height() as f32 {
                let new_coord = UVec2::new(x as u32, y as u32);
                // Only update if the coordinate has changed to avoid redundant dispatches
                if cursor_query.board_coord != Some(new_coord) {
                    trace!(x = new_coord.x, y = new_coord.y, "new board coord");
                    cursor_query.board_coord = Some(new_coord);
                }
                return;
            }
        } else {
            warn!("Could not convert screen position to world position");
        }
    }

    // If cursor is outside the window or the board, clear the query
    cursor_query.board_coord = None;
}

/// System 2: Dispatch the compute shader when a new query is requested
pub fn dispatch_id_query(
    query_request: Res<CursorIDQuery>,
    mut worker: ResMut<AppComputeWorker<PlayerIdWorker>>,
) {
    // Only run if the query request has changed since last time we checked
    if !query_request.is_changed() {
        return;
    }

    // Don't dispatch a new query if the GPU is already busy with the last one
    if worker.state.is_running() {
        return;
    }

    if let Some(board_coord) = query_request.board_coord {
        // Write the new coordinate to the GPU uniform buffer
        if worker
            .try_write_slice("target_coord", &[board_coord.x, board_coord.y])
            .is_err()
        {
            warn!("Could not write to GPU uniform buffer");
        } else {
            // Execute the compute shader
            worker.execute();
        }
    }
}

/// System 3: Check for and collect the result from the GPU when it's ready
pub fn process_id_query_result(
    mut result: ResMut<CursorIDResult>,
    worker: Res<AppComputeWorker<PlayerIdWorker>>,
) {
    // The `one_shot` worker will be `ready()` for one frame after the GPU finishes
    if worker.ready() {
        const NOT_FOUND: u32 = 0xFFFFFFFF;
        let result_vec = worker.read_vec::<u32>("result_id");
        let id = result_vec.first().copied().unwrap_or(NOT_FOUND);

        if id == NOT_FOUND {
            result.player_id = None;
        } else {
            result.player_id = Some(id);
        }
    }
}

/// Plugin that manages cursor-based player ID queries.
///
/// This plugin sets up the complete cursor query pipeline:
/// - Registers the [`PlayerIdWorker`] compute worker
/// - Initializes [`CursorIDQuery`] and [`CursorIDResult`] resources
/// - Adds systems that track cursor position, dispatch GPU queries, and collect results
///
/// The three systems run in a chain each frame:
/// 1. [`update_cursor_query`] - Converts cursor position to board coordinates
/// 2. [`dispatch_id_query`] - Dispatches GPU compute shader when coordinate changes
/// 3. [`process_id_query_result`] - Reads back the player ID from GPU when ready
///
/// ## Example
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use openfrust::shaders::compute::cursor_worker::CursorQueryPlugin;
/// App::new()
///     .add_plugins(CursorQueryPlugin)
///     .run();
/// ```
pub struct CursorQueryPlugin;

impl Plugin for CursorQueryPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AppComputeWorkerPlugin::<PlayerIdWorker>::default())
            .init_resource::<CursorIDQuery>()
            .init_resource::<CursorIDResult>()
            .add_systems(
                Update,
                (
                    update_cursor_query,
                    dispatch_id_query,
                    process_id_query_result,
                )
                    .chain(),
            );
    }
}
