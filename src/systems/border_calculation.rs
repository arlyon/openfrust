use bevy::prelude::*;

use crate::types::*;
use crate::utils::get_neighbors;
use crate::{BOARD_HEIGHT, BOARD_WIDTH};

/// Incrementally update borders when a tile at (x, y) changes from old_owner to new_owner
/// This is much faster than recalculating all borders
pub fn update_borders_incremental(
    x: usize,
    y: usize,
    old_owner: PlayerId,
    new_owner: PlayerId,
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
) {
    // Remove (x, y) from old owner's border tiles if applicable
    if old_owner != NO_OWNER {
        if let Some((_, mut old_player)) = players.iter_mut().find(|(_, p)| p.id == old_owner) {
            old_player.border_tiles.remove(&(x, y));
        }
    }

    // Check if (x, y) is now a border tile for the new owner
    if new_owner != NO_OWNER {
        let is_border = get_neighbors(x, y)
            .iter()
            .any(|&(nx, ny)| board.tiles[ny][nx].owner != new_owner);

        if is_border {
            if let Some((_, mut new_player)) = players.iter_mut().find(|(_, p)| p.id == new_owner) {
                new_player.border_tiles.insert((x, y));
            }
        }
    }

    // Update neighbors of the conquered tile
    for (nx, ny) in get_neighbors(x, y) {
        let neighbor_owner = board.tiles[ny][nx].owner;

        if neighbor_owner == NO_OWNER {
            continue;
        }

        // Check if neighbor is now a border tile
        let is_neighbor_border = get_neighbors(nx, ny)
            .iter()
            .any(|&(nnx, nny)| board.tiles[nny][nnx].owner != neighbor_owner);

        if let Some((_, mut neighbor_player)) =
            players.iter_mut().find(|(_, p)| p.id == neighbor_owner)
        {
            if is_neighbor_border {
                neighbor_player.border_tiles.insert((nx, ny));
            } else {
                neighbor_player.border_tiles.remove(&(nx, ny));
            }
        }
    }
}

/// System to recalculate borders on startup
#[tracing::instrument(skip_all)]
pub fn initial_border_calculation(
    mut players: Query<&mut PlayerData, With<Alive>>,
    board: Res<Board>,
) {
    for mut player in players.iter_mut() {
        player.border_tiles.clear();
    }

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let owner_id = board.tiles[y][x].owner;
            if owner_id != NO_OWNER {
                let is_border = get_neighbors(x, y)
                    .iter()
                    .any(|&(nx, ny)| board.tiles[ny][nx].owner != owner_id);
                if is_border {
                    if let Some(mut player) = players.iter_mut().find(|p| p.id == owner_id) {
                        player.border_tiles.insert((x, y));
                    }
                }
            }
        }
    }
}

/// Recalculates all player borders based on current tile ownership
#[tracing::instrument(skip_all)]
pub fn recalculate_all_borders(
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
) {
    for (_, mut player) in players.iter_mut() {
        player.border_tiles.clear();
    }

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let owner_id = board.tiles[y][x].owner;
            if owner_id != NO_OWNER {
                let is_border = get_neighbors(x, y)
                    .iter()
                    .any(|&(nx, ny)| board.tiles[ny][nx].owner != owner_id);
                if is_border {
                    if let Some((_, mut player)) =
                        players.iter_mut().find(|(_, p)| p.id == owner_id)
                    {
                        player.border_tiles.insert((x, y));
                    }
                }
            }
        }
    }
}
