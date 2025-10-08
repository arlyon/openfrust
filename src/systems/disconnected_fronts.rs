use bevy::prelude::*;

use crate::types::*;
use crate::utils::get_neighbors;

/// Clear expansion fronts between players that no longer share a border and refund troops
#[tracing::instrument(skip_all)]
pub fn clear_disconnected_fronts(
    board: &Board,
    expansions: &mut ActiveExpansions,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
) {
    for a in 0..crate::NUM_ENTITIES {
        for b in (a + 1)..crate::NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Check if these two players still share a border
            // Optimized: Use border_tiles instead of full board scan
            let shares_border = check_players_share_border(a, b, board, players);

            if !shares_border {
                // Refund troops to both players
                if a != NO_OWNER {
                    if let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == a) {
                        let refund = if net_troops > 0 { net_troops as u32 } else { 0 };
                        player.troops += refund;
                        if refund > 0 {
                            bevy::log::info!("Refunded {} troops to Player {}", refund, a);
                        }
                    }
                }

                if b != NO_OWNER {
                    if let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == b) {
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
                }

                expansions.clear_border(a, b);

                // Also remove the conquest queue for this border
                expansions.conquer_queues.remove(&(a, b));
                expansions.conquer_queues.remove(&(b, a));
            }
        }
    }
}

/// Optimized border checking using player border_tiles instead of full board scan
fn check_players_share_border(
    a: PlayerId,
    b: PlayerId,
    board: &Board,
    players: &Query<(Entity, &mut PlayerData), With<Alive>>,
) -> bool {
    // Special case: If either is NO_OWNER (wilderness), check if any player borders wilderness
    if a == NO_OWNER {
        // Check if player b has any border tiles adjacent to wilderness
        if let Some((_, player_b)) = players.iter().find(|(_, p)| p.id == b) {
            for &(x, y) in &player_b.border_tiles {
                for (nx, ny) in get_neighbors(x, y) {
                    if board.get(nx, ny).owner() as usize == NO_OWNER {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    if b == NO_OWNER {
        // Check if player a has any border tiles adjacent to wilderness
        if let Some((_, player_a)) = players.iter().find(|(_, p)| p.id == a) {
            for &(x, y) in &player_a.border_tiles {
                for (nx, ny) in get_neighbors(x, y) {
                    if board.get(nx, ny).owner() as usize == NO_OWNER {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    // Both are players: check if player A's border tiles are adjacent to player B
    if let Some((_, player_a)) = players.iter().find(|(_, p)| p.id == a) {
        for &(x, y) in &player_a.border_tiles {
            for (nx, ny) in get_neighbors(x, y) {
                if board.get(nx, ny).owner() as usize == b {
                    return true;
                }
            }
        }
    }

    false
}
