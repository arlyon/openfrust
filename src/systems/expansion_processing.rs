use bevy::prelude::*;
use rand::Rng;
use std::collections::BinaryHeap;

use crate::types::*;
use crate::utils::get_neighbors;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, EXPANSION_RATE_BASE};

/// Process all expansion fronts and move borders based on relative troop counts
#[tracing::instrument(skip_all)]
pub fn process_expansion_fronts(board: &mut Board, expansions: &mut ActiveExpansions) {
    let _span = tracing::info_span!("process_expansion_fronts").entered();

    let mut rng = rand::rng();

    // Process each border pair
    {
        let _process_span = tracing::debug_span!("process_borders").entered();
        for a in 0..crate::NUM_ENTITIES {
            for b in (a + 1)..crate::NUM_ENTITIES {
                let net_troops = expansions.get_net_troops(a, b);

                if net_troops == 0 {
                    continue;
                }

                // Determine attacker and defender based on sign
                let (attacker, defender) = if net_troops > 0 { (a, b) } else { (b, a) };
                let velocity = net_troops.abs();

                // Calculate how many tiles to conquer this tick based on troop advantage
                let tiles_to_move =
                    (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as usize;

                // Get or create the priority queue for this border
                let key = (attacker, defender);
                let queue = expansions
                    .conquer_queues
                    .entry(key)
                    .or_insert_with(BinaryHeap::new);

                // If queue is empty, seed it with border tiles
                if queue.is_empty() {
                    for y in 0..BOARD_HEIGHT {
                        for x in 0..BOARD_WIDTH {
                            if board.tiles[y][x].owner == attacker {
                                add_neighbors_to_queue(
                                    x, y, attacker, defender, board, queue, &mut rng,
                                );
                            }
                        }
                    }
                }

                // Process the priority queue
                let mut conquered_this_tick = 0;
                let mut newly_conquered = Vec::new();

                while conquered_this_tick < tiles_to_move {
                    if let Some(task) = queue.pop() {
                        // Double-check tile is still owned by defender
                        if board.tiles[task.y][task.x].owner == defender {
                            // Conquer the tile
                            board.tiles[task.y][task.x].owner = attacker;
                            conquered_this_tick += 1;
                            newly_conquered.push((task.x, task.y));
                        }
                    } else {
                        break; // No more tiles to conquer
                    }
                }

                // Add neighbors of newly conquered tiles to the queue
                for (x, y) in newly_conquered {
                    add_neighbors_to_queue(x, y, attacker, defender, board, queue, &mut rng);
                }
            }
        }
    }

    // Reduce troop counts for each tick (troops are consumed as they push)
    {
        let _decay_span = tracing::debug_span!("decay_troops").entered();
        for troops in expansions.fronts.iter_mut() {
            if *troops != 0 {
                let abs_troops = troops.abs();
                let decay_rate = ((abs_troops as f32 * 0.1).max(1.0) as i32).min(abs_troops);
                *troops = if *troops > 0 {
                    troops.saturating_sub(decay_rate)
                } else {
                    troops.saturating_add(decay_rate)
                };
            }
        }
    }

    // Clean up empty queues
    expansions
        .conquer_queues
        .retain(|_, queue| !queue.is_empty());
}

/// Add neighboring tiles to the conquest priority queue
fn add_neighbors_to_queue(
    x: usize,
    y: usize,
    attacker: PlayerId,
    defender: PlayerId,
    board: &Board,
    queue: &mut BinaryHeap<ConquerTask>,
    rng: &mut rand::rngs::ThreadRng,
) {
    for (nx, ny) in get_neighbors(x, y) {
        if board.tiles[ny][nx].owner == defender {
            // Count how many neighbors are owned by attacker (encourages front-line expansion)
            let mut num_owned_by_attacker = 0;
            for (nnx, nny) in get_neighbors(nx, ny) {
                if board.tiles[nny][nnx].owner == attacker {
                    num_owned_by_attacker += 1;
                }
            }

            // Terrain difficulty
            let terrain_mag = board.tiles[ny][nx].terrain_difficulty;

            // Priority calculation: prefer tiles with fewer friendly neighbors (pushes front line)
            // and easier terrain
            let random_factor = rng.random_range(10..=17);
            let priority = (random_factor as f32
                * (1.0 - num_owned_by_attacker as f32 * 0.5 + terrain_mag / 2.0))
                as u32;

            queue.push(ConquerTask {
                priority,
                x: nx,
                y: ny,
            });
        }
    }
}
