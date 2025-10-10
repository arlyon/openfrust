use bevy::prelude::*;
use bevy_app_compute::prelude::*;

use super::ExpansionWorker;
use crate::types::Board;

/// Sync the CPU board state to GPU after initial setup
pub fn sync_board_to_gpu(board: Res<Board>, mut worker: ResMut<AppComputeWorker<ExpansionWorker>>) {
    // Pack two tiles into each u32 for better memory efficiency
    let board_data: Vec<u32> = board
        .tiles
        .chunks_exact(2)
        .map(|chunk| {
            let tile1 = u32::from(chunk[0].0); // Lower 16 bits
            let tile2 = u32::from(chunk[1].0); // Upper 16 bits
            (tile2 << 16) | tile1
        })
        .collect();

    // Write to both ping-pong buffers and render buffer so they start in sync
    worker.write_slice("board_in", &board_data);
    worker.write_slice("board_out", &board_data);

    info!(
        "Synced initial board state to GPU ({} tiles)",
        board_data.len()
    );
}
