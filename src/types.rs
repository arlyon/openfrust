use bevy::prelude::*;
use bitfield::bitfield;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::{NUM_ENTITIES, NUM_PAIRS};

pub type PlayerId = usize;
pub const NO_OWNER: PlayerId = 0;

/// Message sent when a tile changes ownership (buffered for rendering)
#[derive(Message)]
pub struct TileChangeMessage {
    pub x: usize,
    pub y: usize,
    pub new_owner: PlayerId,
}

bitfield! {
    /// Represents a single tile on the game board using a compact bitfield.
    #[derive(Clone, Copy)]
    pub struct Tile(u16);
    impl Debug;
    pub u16, owner, set_owner: 11, 0;           // 12 bits for owner ID (up to 4096 players)
    pub u8, terrain_diff, set_terrain_diff: 14, 12; // 3 bits for terrain type (8 types)
    pub has_fallout, set_fallout: 15;           // 1 bit for fallout flag
}

impl Tile {
    pub fn new(owner: PlayerId, terrain_difficulty: f32) -> Self {
        let mut tile = Tile(0);
        tile.set_owner(owner as u16);
        // Map terrain difficulty to 0-7 range
        let terrain_type = ((terrain_difficulty.clamp(0.5, 2.0) - 0.5) / 1.5 * 7.0) as u8;
        tile.set_terrain_diff(terrain_type);
        tile.set_fallout(false);
        tile
    }

    pub fn terrain_difficulty(&self) -> f32 {
        // Map 0-7 range back to terrain difficulty
        0.5 + (self.terrain_diff() as f32 / 7.0) * 1.5
    }
}

/// Represents a player in the game.
#[derive(Debug, Clone, Component)]
pub struct PlayerData {
    pub id: PlayerId,
    pub char: char,
    pub troops: u32,
    pub tile_count: usize,
    pub sum_x: u64, // Sum of x coordinates of all owned tiles (for center calculation)
    pub sum_y: u64, // Sum of y coordinates of all owned tiles (for center calculation)
    pub border_tiles: HashSet<(usize, usize)>,
    pub color: Color,
}

/// Marker component for alive players
#[derive(Component)]
pub struct Alive;

/// Resource holding the game board (flattened for cache efficiency)
#[derive(Resource)]
pub struct Board {
    pub tiles: Vec<Tile>,
    pub width: usize,
    pub height: usize,
}

impl Board {
    /// Create a new board with the given dimensions
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            tiles: vec![Tile::new(NO_OWNER, 1.0); width * height],
            width,
            height,
        }
    }

    /// Get a tile at the given coordinates
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> &Tile {
        &self.tiles[y * self.width + x]
    }

    /// Get a mutable tile at the given coordinates
    #[inline]
    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut Tile {
        &mut self.tiles[y * self.width + x]
    }

    /// Get the index for the given coordinates
    #[inline]
    pub fn index(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }
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

/// Resource to map PlayerId to Color for fast lookups
#[derive(Resource)]
pub struct PlayerColorMap(pub Vec<Color>);

/// Resource to map PlayerId to Entity for O(1) lookups
#[derive(Resource)]
pub struct PlayerEntityMap(pub Vec<Option<Entity>>);
