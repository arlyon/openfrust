use bevy::prelude::*;

use crate::types::*;
use crate::utils::get_neighbors;
use crate::{BOARD_HEIGHT, BOARD_WIDTH};

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
