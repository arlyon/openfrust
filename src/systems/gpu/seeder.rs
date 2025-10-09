use bevy::prelude::*;
use bevy_app_compute::prelude::*;

use super::worker::{ExpansionWorker, SimParams};
use crate::types::BoardSeeds;

/// Startup system that seeds the board on GPU from sparse starting positions
#[tracing::instrument(skip_all)]
pub fn seed_board_and_sync(
    seeds: Res<BoardSeeds>,
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
) {
    info!("Seeding board on GPU with {} starting positions...", seeds.0.len());

    // Debug: print first few seeds
    for (i, seed) in seeds.0.iter().take(3).enumerate() {
        info!("Seed {}: pos=({}, {}), data={:#010x}", i, seed.pos.x, seed.pos.y, seed.data);
    }

    // Write the sparse seed data to the GPU buffer
    worker.write_slice("seeds", &seeds.0);

    // Update the seed_count in the params uniform so the shader knows how many seeds to read
    worker.write_slice("params", &[SimParams {
        board_width: crate::BOARD_WIDTH as u32,
        board_height: crate::BOARD_HEIGHT as u32,
        expansion_rate: crate::EXPANSION_RATE_BASE,
        num_entities: crate::NUM_ENTITIES as u32,
        seed_count: seeds.0.len() as u32,
        _padding1: 0,
        _padding2: 0,
        _padding3: 0,
    }]);

    // The worker will automatically run at the end of the startup frame:
    // 1. Seed pass populates board_in with starting positions
    // 2. Expansion pass reads board_in (seeded!), writes board_out
    // 3. Swap occurs for ping-pong buffering
    // 4. Stats/adjacency passes run on the board_out data
}

/// System to reset seed_count after first frame (prevents re-seeding)
/// Uses a local resource to track if we've already reset
#[derive(Resource, Default)]
pub struct SeedingComplete(bool);

pub fn reset_seed_count(
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
    mut seeding_complete: Local<bool>,
) {
    // Only run once
    if !*seeding_complete {
        // Reset seed_count to 0 so seeding shader doesn't run again
        worker.write_slice("params", &[SimParams {
            board_width: crate::BOARD_WIDTH as u32,
            board_height: crate::BOARD_HEIGHT as u32,
            expansion_rate: crate::EXPANSION_RATE_BASE,
            num_entities: crate::NUM_ENTITIES as u32,
            seed_count: 0, // Reset to 0 so seeding doesn't run again
            _padding1: 0,
            _padding2: 0,
            _padding3: 0,
        }]);
        *seeding_complete = true;
        info!("Seed count reset. Board seeding complete.");
    }
}
