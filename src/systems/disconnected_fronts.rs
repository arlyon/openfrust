use bevy::prelude::*;

use super::gpu_orchestrator::AdjacencyMatrix;
use crate::types::*;
use crate::{NUM_ENTITIES};

/// Clear expansion fronts between players that no longer share a border and refund troops
#[tracing::instrument(skip_all)]
pub fn clear_disconnected_fronts(
    expansions: &mut ActiveExpansions,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    adjacency: &AdjacencyMatrix,
) {
    for a in 0..crate::NUM_ENTITIES {
        for b in (a + 1)..crate::NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Check if these two players still share a border using adjacency matrix
            let shares_border = adjacency.0[a * NUM_ENTITIES + b] == 1;

            if !shares_border {
                // Refund troops to both players
                if a != NO_OWNER
                    && let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == a)
                {
                    let refund = if net_troops > 0 { net_troops as u32 } else { 0 };
                    player.troops += refund;
                    if refund > 0 {
                        bevy::log::info!("Refunded {} troops to Player {}", refund, a);
                    }
                }

                if b != NO_OWNER
                    && let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == b)
                {
                    let refund = if net_troops < 0 {
                        (-net_troops) as u32
                    } else {
                        0
                    };
                    player.troops += refund;
                    if refund > 0 {
                        bevy::log::info!("Refunded {} troops to Player {}", refund, b);
                    }
                }

                expansions.clear_border(a, b);

                // Also remove the conquest queue for this border
                expansions.conquer_queues.remove(&(a, b));
                expansions.conquer_queues.remove(&(b, a));
            }
        }
    }
}
