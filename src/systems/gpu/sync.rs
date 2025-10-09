use bevy::prelude::*;
use bevy_app_compute::prelude::*;

use super::ExpansionWorker;
use crate::types::Board;

/// Sync the CPU board state to GPU after initial setup
pub fn sync_board_to_gpu(board: Res<Board>, mut worker: ResMut<AppComputeWorker<ExpansionWorker>>) {
    // Convert board tiles to u32 for GPU
    let board_data: Vec<u32> = board.tiles.iter().map(|t| t.0 as u32).collect();

    // Write to both ping-pong buffers and render buffer so they start in sync
    worker.write_slice("board_in", &board_data);
    worker.write_slice("board_out", &board_data);

    info!("Synced initial board state to GPU ({} tiles)", board_data.len());
}
