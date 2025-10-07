use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::{NUM_ENTITIES, NUM_PAIRS};

pub type PlayerId = usize;
pub const NO_OWNER: PlayerId = 0;

/// Represents a single tile on the game board.
#[derive(Clone, Copy, Debug)]
pub struct Tile {
    pub owner: PlayerId,
    /// Terrain difficulty multiplier (1.0 = normal)
    pub terrain_difficulty: f32,
}

/// Represents a player in the game.
#[derive(Debug, Clone, Component)]
pub struct PlayerData {
    pub id: PlayerId,
    pub char: char,
    pub troops: u32,
    pub tile_count: usize,
    pub border_tiles: HashSet<(usize, usize)>,
    pub color: Color,
}

/// Marker component for alive players
#[derive(Component)]
pub struct Alive;

/// Resource holding the game board
#[derive(Resource)]
pub struct Board {
    pub tiles: Vec<Vec<Tile>>,
}

/// Component linking player info text to player entity
#[derive(Component)]
pub struct PlayerInfoText {
    pub player_entity: Entity,
}

/// Represents a tile to be conquered in priority order
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ConquerTask {
    pub priority: u32,
    pub x: usize,
    pub y: usize,
}

impl Ord for ConquerTask {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority) // Min-heap: lower priority processed first
    }
}

impl PartialOrd for ConquerTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Resource tracking all active expansion fronts
/// Uses a triangular array where index = X*N + (Y-X-1) for pair (X,Y) where X < Y
/// Positive values mean X is pushing into Y, negative means Y is pushing into X
#[derive(Resource)]
pub struct ActiveExpansions {
    pub fronts: [i32; NUM_PAIRS],
    /// Priority queues for each border expansion
    pub conquer_queues: HashMap<(PlayerId, PlayerId), BinaryHeap<ConquerTask>>,
}

impl Default for ActiveExpansions {
    fn default() -> Self {
        Self {
            fronts: [0; NUM_PAIRS],
            conquer_queues: HashMap::new(),
        }
    }
}

impl ActiveExpansions {
    /// Calculate array index for a pair of players
    /// Formula: N*x - (x*(x+1))/2 + y - x - 1 where X < Y
    pub fn pair_index(a: PlayerId, b: PlayerId) -> usize {
        let (x, y) = if a < b { (a, b) } else { (b, a) };
        NUM_ENTITIES * x - (x * (x + 1)) / 2 + y - x - 1
    }

    /// Add troops to a border, canceling out opposing forces
    pub fn add_troops(&mut self, attacker: PlayerId, defender: PlayerId, troops: i32) {
        let idx = Self::pair_index(attacker, defender);
        let multiplier = if attacker < defender { 1 } else { -1 };
        self.fronts[idx] += troops * multiplier;
    }

    /// Get net troops for a border (positive means lower ID is winning)
    pub fn get_net_troops(&self, a: PlayerId, b: PlayerId) -> i32 {
        let idx = Self::pair_index(a, b);
        self.fronts[idx]
    }

    /// Clear a specific border
    pub fn clear_border(&mut self, a: PlayerId, b: PlayerId) {
        let idx = Self::pair_index(a, b);
        self.fronts[idx] = 0;
    }

    /// Remove all borders involving a specific player
    pub fn remove_player(&mut self, player_id: PlayerId) {
        for other_id in 0..NUM_ENTITIES {
            if other_id != player_id {
                self.clear_border(player_id, other_id);
            }
        }
    }
}

/// Component for tile entities
#[derive(Component)]
pub struct TileEntity {
    pub x: usize,
    pub y: usize,
}

/// Timer for game updates
#[derive(Resource)]
pub struct GameUpdateTimer(pub Timer);
