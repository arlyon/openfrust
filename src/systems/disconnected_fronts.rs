use bevy::prelude::*;

use crate::NUM_ENTITIES;
use crate::types::{ActiveExpansions, Alive, NO_OWNER, PlayerData, PlayerId};

/// Clear expansion fronts between players that no longer share a border and refund troops
#[tracing::instrument(skip_all)]
pub fn clear_disconnected_fronts(
    expansions: &mut ActiveExpansions,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    adjacency: &[u32],
) {
    for a in 0..crate::NUM_ENTITIES {
        for b in (a + 1)..crate::NUM_ENTITIES {
            let a = PlayerId::new_unchecked(a);
            let b = PlayerId::new_unchecked(b);
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Check if these two players still share a border using adjacency matrix
            let shares_border =
                adjacency[(u16::from(a) * NUM_ENTITIES + u16::from(b)) as usize] == 1;

            if !shares_border {
                // Refund troops to both players
                if a != NO_OWNER
                    && let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == a)
                {
                    let refund = if net_troops > 0 {
                        net_troops.cast_unsigned()
                    } else {
                        0
                    };
                    player.troops += refund;
                    if refund > 0 {
                        bevy::log::info!("Refunded {} troops to Player {}", refund, a);
                    }
                }

                if b != NO_OWNER
                    && let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == b)
                {
                    let refund = if net_troops < 0 {
                        net_troops.cast_unsigned()
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
