use bevy::prelude::*;

use crate::NUM_ENTITIES;
use crate::types::{ActiveExpansions, Alive, NO_OWNER, PlayerData, PlayerEntityMap, PlayerId};

/// Checks the bit-packed adjacency array to see if two players border each other
fn are_adjacent(adjacency_packed: &[u32], p1: PlayerId, p2: PlayerId) -> bool {
    let n = NUM_ENTITIES as u32;
    let p1_u32 = u16::from(p1) as u32;
    let p2_u32 = u16::from(p2) as u32;

    if p1_u32 == p2_u32 {
        return false;
    }

    let x = p1_u32.min(p2_u32);
    let y = p1_u32.max(p2_u32);

    // This formula must be identical to the one in the shader and ActiveExpansions::pair_index
    let linear_bit_index = (n * x - (x * (x + 1)) / 2 + y - x - 1) as usize;

    let word_index = linear_bit_index / 32;
    let bit_in_word = linear_bit_index % 32;

    if word_index >= adjacency_packed.len() {
        return false; // Should not happen if everything is sized correctly
    }

    // Check if the specific bit is set
    (adjacency_packed[word_index] >> bit_in_word) & 1 == 1
}

/// Clear expansion fronts between players that no longer share a border and refund troops
#[tracing::instrument(skip_all)]
pub fn clear_disconnected_fronts(
    expansions: &mut ActiveExpansions,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>, // Query just for PlayerData
    adjacency: &[u32],                                          // Packed bitfield
    player_map: &PlayerEntityMap,                               // O(1) lookup map
) {
    for a in 0..crate::NUM_ENTITIES {
        for b in (a + 1)..crate::NUM_ENTITIES {
            let a = PlayerId::new_unchecked(a);
            let b = PlayerId::new_unchecked(b);
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Check if these two players still share a border using the packed adjacency
            let shares_border = are_adjacent(adjacency, a, b);

            if !shares_border {
                // Refund troops to both players using O(1) entity lookup
                if a != NO_OWNER {
                    if let Some(entity) = player_map.0[usize::from(a)] {
                        if let Ok((_, mut player)) = players.get_mut(entity) {
                            let refund = if net_troops > 0 {
                                net_troops.cast_unsigned()
                            } else {
                                0
                            };
                            player.troops += refund;
                            if refund > 0 {
                                bevy::log::debug!("Refunded {} troops to Player {}", refund, a);
                            }
                        }
                    }
                }

                if b != NO_OWNER {
                    if let Some(entity) = player_map.0[usize::from(b)] {
                        if let Ok((_, mut player)) = players.get_mut(entity) {
                            let refund = if net_troops < 0 {
                                (-net_troops) as u32
                            } else {
                                0
                            };
                            player.troops += refund;
                            if refund > 0 {
                                bevy::log::debug!("Refunded {} troops to Player {}", refund, b);
                            }
                        }
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
