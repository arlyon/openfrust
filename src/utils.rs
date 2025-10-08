use crate::{BOARD_HEIGHT, BOARD_WIDTH};

pub fn get_neighbors(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0 && nx < BOARD_WIDTH as isize && ny >= 0 && ny < BOARD_HEIGHT as isize {
                neighbors.push((nx as usize, ny as usize));
            }
        }
    }
    neighbors
}
