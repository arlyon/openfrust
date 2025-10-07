use bevy::prelude::*;
use rand::Rng;
use std::collections::BinaryHeap;

use super::border_calculation::update_borders_incremental;
use crate::types::*;
use crate::utils::get_neighbors;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, EXPANSION_RATE_BASE};

/// Process all expansion fronts and move borders based on relative troop counts
#[tracing::instrument(skip_all)]
pub fn process_expansion_fronts(
    board: &mut Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
    mut tile_change_writer: MessageWriter<TileChangeMessage>,
) {
    let mut rng = rand::rng();

    // Process each border pair
    {
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
                // Optimized: Use attacker's border_tiles instead of scanning entire board
                if queue.is_empty() {
                    if let Some((_, attacker_player)) =
                        players.iter().find(|(_, p)| p.id == attacker)
                    {
                        for &(bx, by) in &attacker_player.border_tiles {
                            // Check neighbors of this border tile
                            for (nx, ny) in get_neighbors(bx, by) {
                                if board.tiles[ny][nx].owner == defender {
                                    // This neighbor tile can be conquered - add to queue
                                    let mut num_owned_by_attacker = 0;
                                    for (nnx, nny) in get_neighbors(nx, ny) {
                                        if board.tiles[nny][nnx].owner == attacker {
                                            num_owned_by_attacker += 1;
                                        }
                                    }

                                    let terrain_mag = board.tiles[ny][nx].terrain_difficulty;
                                    let random_factor = rng.random_range(10..=17);
                                    let priority = (random_factor as f32
                                        * (1.0 - num_owned_by_attacker as f32 * 0.5
                                            + terrain_mag / 2.0))
                                        as u32;

                                    queue.push(ConquerTask {
                                        priority,
                                        x: nx,
                                        y: ny,
                                    });
                                }
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
                            let old_owner = defender;

                            // Conquer the tile
                            board.tiles[task.y][task.x].owner = attacker;

                            // Send tile change message for rendering
                            tile_change_writer.write(TileChangeMessage {
                                x: task.x,
                                y: task.y,
                                new_owner: attacker,
                            });

                            // Update tile counts and coordinate sums incrementally
                            for (_, mut player) in players.iter_mut() {
                                if player.id == attacker {
                                    player.tile_count += 1;
                                    player.sum_x += task.x as u64;
                                    player.sum_y += task.y as u64;
                                } else if player.id == defender {
                                    player.tile_count -= 1;
                                    player.sum_x -= task.x as u64;
                                    player.sum_y -= task.y as u64;
                                }
                            }

                            // Update borders incrementally instead of full recalculation
                            update_borders_incremental(
                                task.x, task.y, old_owner, attacker, board, players,
                            );

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
