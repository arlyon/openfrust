use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, ParallelSlice};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::*;
use crate::utils::get_neighbors;

/// AI assigns troops to expansion fronts and logs active fronts
#[tracing::instrument(skip_all)]
pub fn assign_and_log_expansions(
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
    pool: &ComputeTaskPool,
) {
    // Assign troops to expansion fronts in parallel
    {
        // Collect all players into a Vec to create a mutable slice
        let all_players: Vec<Mut<PlayerData>> = players.iter_mut().map(|(_, p)| p).collect();
        let assignments = Mutex::new(Vec::new());

        // Use par_chunks_mut for safe, parallel mutation of player data and calculation
        all_players.par_chunk_map(pool, 128, |_idx, chunk| {
            let mut local_assignments = Vec::new();
            for player in chunk {
                if player.troops > 50 {
                    // Call a pure calculation function
                    local_assignments.extend(calculate_player_assignments(board, player));
                }
            }
            if !local_assignments.is_empty() {
                assignments.lock().unwrap().extend(local_assignments);
            }
        });

        // --- Serial Apply Phase ---
        for (attacker, defender, troops) in assignments.into_inner().unwrap() {
            expansions.add_troops(attacker, defender, troops);
        }
    }

    // Log active expansion fronts
    {
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
/// Returns a list of (attacker, defender, troops) assignments to be applied
fn calculate_player_assignments(
    board: &Board,
    player: &PlayerData,
) -> Vec<(PlayerId, PlayerId, i32)> {
    if player.border_tiles.is_empty() || player.troops < 10 {
        return Vec::new();
    }

    // Count neighbors for each border type
    let mut neighbor_counts: HashMap<PlayerId, usize> = HashMap::new();

    for &(bx, by) in &player.border_tiles {
        for (nx, ny) in get_neighbors(bx, by) {
            let neighbor_owner = board.get(nx, ny).owner() as usize;
            if neighbor_owner != player.id && neighbor_owner == NO_OWNER {
                *neighbor_counts.entry(neighbor_owner).or_insert(0) += 1;
            }
        }
    }

    if neighbor_counts.is_empty() {
        return Vec::new();
    }

    // Assign half of available troops to expansion fronts proportionally
    let troops_to_assign = player.troops / 2;
    let total_border_length: usize = neighbor_counts.values().sum();

    let mut assignments = Vec::new();
    for (neighbor_id, border_length) in neighbor_counts {
        let proportion = border_length as f32 / total_border_length as f32;
        let troops = (troops_to_assign as f32 * proportion) as i32;

        if troops > 0 {
            assignments.push((player.id, neighbor_id, troops));
        }
    }
    assignments
}
