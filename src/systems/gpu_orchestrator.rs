use bevy::prelude::*;
use bevy_app_compute::prelude::*;

use super::gpu::ExpansionWorker;
use super::sim_manager::{SimManager, SimStatus};
use crate::types::{ActiveExpansions, Alive, PlayerData, PlayerEntityMap, PlayerInfoText};

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
pub fn gpu_orchestrator(
    // The manager that now holds all the state and logic
    mut sim_manager: ResMut<SimManager>,
    // Dependencies required by the manager's tick method
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    player_map: Res<PlayerEntityMap>,
    time: Res<Time>,
) {
    tracing::info!("READY! {} {:?}", worker.ready(), worker.state);

    // Execute one simulation tick through the manager
    let status = sim_manager.tick(
        &mut worker,
        &mut players,
        &mut expansions,
        &mut commands,
        &text_query,
        &player_map,
        &time,
    );

    // The manager already logs warmup messages, but we log stalls here
    // since they're potentially problematic and worth highlighting
    if status == SimStatus::Stalled {
        bevy::log::warn!("GPU is busy, simulation tick stalled.");
    }
}
