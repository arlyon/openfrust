use crate::{BOARD_HEIGHT, BOARD_WIDTH};

/// Returns a zero-allocation iterator over the valid neighbors of a given coordinate.
pub fn get_neighbors(x: usize, y: usize) -> impl Iterator<Item = (usize, usize)> {
    // Create an iterator over all possible relative coordinates (-1,-1) to (1,1)
    (-1..=1)
        .flat_map(move |dy| (-1..=1).map(move |dx| (dx, dy)))
        // Filter out the center point (0,0)
        .filter(|&(dx, dy)| dx != 0 || dy != 0)
        // Map the relative coordinates to absolute board coordinates,
        // filtering out any that are off the board.
        .filter_map(move |(dx, dy)| {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0 && nx < BOARD_WIDTH as isize && ny >= 0 && ny < BOARD_HEIGHT as isize {
                Some((nx as usize, ny as usize))
            } else {
                None
            }
        })
}
