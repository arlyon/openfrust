use bevy::prelude::*;
use std::collections::HashMap;

use crate::types::*;
use crate::utils::get_neighbors;

/// AI assigns troops to expansion fronts and logs active fronts
#[tracing::instrument(skip_all)]
pub fn assign_and_log_expansions(
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
) {
    // Assign troops to expansion fronts
    {
        let _assign_span = tracing::debug_span!("assign_troops").entered();
        for (_, player) in players.iter_mut() {
            if player.troops > 50 {
                assign_expansion_troops(&board, &player, expansions);
            }
        }
    }

    // Log active expansion fronts
    {
        let _log_span = tracing::debug_span!("log_expansions").entered();
        let has_active_fronts = expansions.fronts.iter().any(|&troops| troops != 0);
        if has_active_fronts {
            bevy::log::debug!("Active expansion fronts:");
            for a in 0..crate::NUM_ENTITIES {
                for b in (a + 1)..crate::NUM_ENTITIES {
                    let net_troops = expansions.get_net_troops(a, b);
                    if net_troops != 0 {
                        let (attacker, defender, troops) = if net_troops > 0 {
                            (a, b, net_troops)
                        } else {
                            (b, a, -net_troops)
                        };
                        let defender_name = if defender == NO_OWNER {
                            "Empty".to_string()
                        } else {
                            format!("Player {}", defender)
                        };
                        let attacker_name = if attacker == NO_OWNER {
                            "Empty".to_string()
                        } else {
                            format!("Player {}", attacker)
                        };
                        bevy::log::debug!(
                            "  {} -> {}: {} troops",
                            attacker_name,
                            defender_name,
                            troops
                        );
                    }
                }
            }
        }
    }
}

/// AI assigns troops to expansion fronts based on border neighbors
fn assign_expansion_troops(board: &Board, player: &PlayerData, expansions: &mut ActiveExpansions) {
    if player.border_tiles.is_empty() || player.troops < 10 {
        return;
    }

    // Count neighbors for each border type
    let mut neighbor_counts: HashMap<PlayerId, usize> = HashMap::new();

    for &(bx, by) in &player.border_tiles {
        for (nx, ny) in get_neighbors(bx, by) {
            let neighbor_owner = board.tiles[ny][nx].owner;
            if neighbor_owner != player.id && neighbor_owner == NO_OWNER {
                *neighbor_counts.entry(neighbor_owner).or_insert(0) += 1;
            }
        }
    }

    if neighbor_counts.is_empty() {
        return;
    }

    // Assign half of available troops to expansion fronts proportionally
    let troops_to_assign = player.troops / 2;
    let total_border_length: usize = neighbor_counts.values().sum();

    for (neighbor_id, border_length) in neighbor_counts {
        let proportion = border_length as f32 / total_border_length as f32;
        let troops = (troops_to_assign as f32 * proportion) as i32;

        if troops > 0 {
            expansions.add_troops(player.id, neighbor_id, troops);
        }
    }
}
