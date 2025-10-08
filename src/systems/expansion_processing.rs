use bevy::prelude::*;
use bevy::tasks::ComputeTaskPool;
use rand::Rng;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};

use super::border_calculation::update_borders_incremental;
use crate::EXPANSION_RATE_BASE;
use crate::types::*;
use crate::utils::get_neighbors;

/// Process all expansion fronts and move borders based on relative troop counts
#[tracing::instrument(skip_all)]
pub fn process_expansion_fronts(
    board: &mut Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
    expansions: &mut ActiveExpansions,
    mut tile_change_writer: MessageWriter<TileChangeMessage>,
    player_map: &PlayerEntityMap,
) {
    let mut rng = rand::rng();

    // --- PARALLEL PHASE: Identify all active fronts ---
    let player_data: Vec<_> = players
        .iter()
        .map(|(_, p)| (p.id, p.border_tiles.clone()))
        .collect();

    let conquests = {
        let _span = tracing::info_span!("calculate").entered();

        let pool = ComputeTaskPool::get();
        let conquests = Arc::new(Mutex::new(Vec::new()));

        const CHUNK_SIZE: usize = 1000;
        let active_fronts: Vec<_> = (0..crate::NUM_ENTITIES)
            .flat_map(|a| (a + 1..crate::NUM_ENTITIES).map(move |b| (a, b)))
            .collect();

        pool.scope(|s| {
            for chunk in active_fronts.chunks(CHUNK_SIZE) {
                s.spawn({
                    let expansions = &expansions;
                    let conquests = conquests.clone();
                    async move {
                        let _span = tracing::info_span!("front_chunk").entered();
                        let mut active_fronts = Vec::new();
                        for (a, b) in chunk {
                            let net_troops = expansions.get_net_troops(*a, *b);

                            if net_troops == 0 {
                                continue;
                            }

                            // Determine attacker and defender based on sign
                            let (attacker, defender) = if net_troops > 0 { (a, b) } else { (b, a) };
                            let velocity = net_troops.abs();

                            // Calculate how many tiles to conquer this tick based on troop advantage
                            let tiles_to_move =
                                (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as usize;

                            active_fronts.push((*attacker, *defender, tiles_to_move));
                        }

                        {
                            let mut conquests = conquests.lock().unwrap();
                            conquests.extend(active_fronts);
                        }
                    }
                });
            }
        });

        Arc::try_unwrap(conquests).unwrap().into_inner().unwrap()
    };

    let _span = tracing::info_span!("apply").entered();

    // --- SERIAL PHASE: Process queues and apply conquests ---
    // TODO: this is actually still very expensive, around 60ms / frame
    for (attacker, defender, tiles_to_move) in conquests {
        let key = (attacker, defender);
        let queue = expansions
            .conquer_queues
            .entry(key)
            .or_insert_with(BinaryHeap::new);

        // If queue is empty, seed it with border tiles
        if queue.is_empty() {
            if let Some((_, border_tiles)) = player_data.iter().find(|(id, _)| *id == attacker) {
                for &(bx, by) in border_tiles {
                    // Check neighbors of this border tile
                    for (nx, ny) in get_neighbors(bx, by) {
                        if board.get(nx, ny).owner() as usize == defender {
                            // This neighbor tile can be conquered - add to queue
                            let mut num_owned_by_attacker = 0;
                            for (nnx, nny) in get_neighbors(nx, ny) {
                                if board.get(nnx, nny).owner() as usize == attacker {
                                    num_owned_by_attacker += 1;
                                }
                            }

                            let terrain_mag = board.get(nx, ny).terrain_difficulty();
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
            }
        }

        // Process the priority queue
        let mut conquered_this_tick = 0;
        let mut newly_conquered = Vec::new();

        while conquered_this_tick < tiles_to_move {
            if let Some(task) = queue.pop() {
                // Double-check tile is still owned by defender
                if board.get(task.x, task.y).owner() as usize == defender {
                    let old_owner = defender;

                    // Conquer the tile
                    board.get_mut(task.x, task.y).set_owner(attacker as u16);

                    // Send tile change message for rendering
                    tile_change_writer.write(TileChangeMessage {
                        x: task.x,
                        y: task.y,
                        new_owner: attacker,
                    });

                    // Update tile counts and coordinate sums incrementally - O(1) lookup
                    if let Some(attacker_entity) = player_map.0[attacker] {
                        if let Ok((_, mut player)) = players.get_mut(attacker_entity) {
                            player.tile_count += 1;
                            player.sum_x += task.x as u64;
                            player.sum_y += task.y as u64;
                        }
                    }
                    if let Some(defender_entity) = player_map.0[defender] {
                        if let Ok((_, mut player)) = players.get_mut(defender_entity) {
                            player.tile_count -= 1;
                            player.sum_x -= task.x as u64;
                            player.sum_y -= task.y as u64;
                        }
                    }

                    // Update borders incrementally instead of full recalculation
                    update_borders_incremental(
                        task.x, task.y, old_owner, attacker, board, players, player_map,
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

    // Reduce troop counts for each tick (troops are consumed as they push)
    {
        let _span = tracing::info_span!("reduction").entered();
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
        if board.get(nx, ny).owner() as usize == defender {
            // Count how many neighbors are owned by attacker (encourages front-line expansion)
            let mut num_owned_by_attacker = 0;
            for (nnx, nny) in get_neighbors(nx, ny) {
                if board.get(nnx, nny).owner() as usize == attacker {
                    num_owned_by_attacker += 1;
                }
            }

            // Terrain difficulty
            let terrain_mag = board.get(nx, ny).terrain_difficulty();

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
