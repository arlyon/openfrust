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
    player_map: &PlayerEntityMap,
) {
    // Remove (x, y) from old owner's border tiles if applicable
    if old_owner != NO_OWNER
        && let Some(entity) = player_map.0[old_owner]
            && let Ok((_, mut old_player)) = players.get_mut(entity) {
                old_player.border_tiles.remove(&(x, y));
            }

    // Check if (x, y) is now a border tile for the new owner
    if new_owner != NO_OWNER {
        let is_border =
            get_neighbors(x, y).any(|(nx, ny)| board.get(nx, ny).owner() as usize != new_owner);

        if is_border
            && let Some(entity) = player_map.0[new_owner]
            && let Ok((_, mut new_player)) = players.get_mut(entity)
        {
            new_player.border_tiles.insert((x, y));
        }
    }

    // Update neighbors of the conquered tile
    for (nx, ny) in get_neighbors(x, y) {
        let neighbor_owner = board.get(nx, ny).owner() as usize;

        if neighbor_owner == NO_OWNER {
            continue;
        }

        // Check if neighbor is now a border tile
        let is_neighbor_border = get_neighbors(nx, ny)
            .any(|(nnx, nny)| board.get(nnx, nny).owner() as usize != neighbor_owner);

        if let Some(entity) = player_map.0[neighbor_owner]
            && let Ok((_, mut neighbor_player)) = players.get_mut(entity)
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
            let owner_id = board.get(x, y).owner() as usize;
            if owner_id != NO_OWNER {
                let is_border = get_neighbors(x, y)
                    .any(|(nx, ny)| board.get(nx, ny).owner() as usize != owner_id);
                if is_border && let Some(mut player) = players.iter_mut().find(|p| p.id == owner_id)
                {
                    player.border_tiles.insert((x, y));
                }
            }
        }
    }
}
