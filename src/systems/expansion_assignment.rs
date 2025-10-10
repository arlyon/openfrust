use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, ParallelSlice};
use std::sync::Mutex;

use crate::types::{ActiveExpansions, Alive, PlayerData, PlayerId};
use crate::{NO_OWNER, NUM_ENTITIES};

/// AI assigns troops to expansion fronts and logs active fronts
#[tracing::instrument(skip_all)]
pub fn assign_and_log_expansions(
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
    adjacency: &[u32],
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
                    local_assignments.extend(calculate_player_assignments(player, adjacency));
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
                    let a = PlayerId::new_unchecked(a);
                    let b = PlayerId::new_unchecked(b);
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
                            format!("Player {defender}")
                        };
                        let attacker_name = if attacker == NO_OWNER {
                            "Empty".to_string()
                        } else {
                            format!("Player {attacker}")
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

/// AI assigns troops to expansion fronts based on adjacency matrix
/// Returns a list of (attacker, defender, troops) assignments to be applied
fn calculate_player_assignments(
    player: &PlayerData,
    adjacency: &[u32],
) -> Vec<(PlayerId, PlayerId, i32)> {
    if player.troops < 10 {
        return Vec::new();
    }

    // Find all neighbors from adjacency matrix
    let mut neighbors = Vec::new();
    for neighbor_id in 0..NUM_ENTITIES {
        let neighbor_id = PlayerId::new_unchecked(neighbor_id);
        if neighbor_id != player.id
            && adjacency[(u16::from(player.id) * NUM_ENTITIES + u16::from(neighbor_id)) as usize]
                == 1
        // Only expand into wilderness
            && neighbor_id == NO_OWNER
        {
            neighbors.push(neighbor_id);
        }
    }

    if neighbors.is_empty() {
        return Vec::new();
    }

    // Assign half of available troops equally to all neighbors
    let troops_to_assign = player.troops / 2;
    let troops_per_neighbor = troops_to_assign / neighbors.len() as u32;

    let mut assignments = Vec::new();
    if troops_per_neighbor > 0 {
        for neighbor_id in neighbors {
            assignments.push((player.id, neighbor_id, troops_per_neighbor as i32));
        }
    }
    assignments
}
