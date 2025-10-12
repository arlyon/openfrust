use bevy::prelude::*;
use bitfield::bitfield;

use crate::{NUM_ENTITIES, NUM_PAIRS};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(u16);

impl PlayerId {
    /// # Panics
    ///
    /// Panics if `id` is greater than `NUM_ENTITIES`
    pub const fn new(id: u16) -> Self {
        assert!(id < NUM_ENTITIES, "PlayerId out of range");
        Self(id)
    }

    pub const fn new_unchecked(id: u16) -> Self {
        Self(id)
    }
}

impl From<PlayerId> for usize {
    fn from(id: PlayerId) -> Self {
        id.0 as usize
    }
}

impl From<PlayerId> for u16 {
    fn from(id: PlayerId) -> Self {
        id.0
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub const NO_OWNER: PlayerId = PlayerId(0);

pub type LivingPlayerUpdate = (With<Alive>, Changed<PlayerData>);

bitfield! {
    /// Represents a single tile on the game board using a compact bitfield.
    #[derive(Clone, Copy)]
    pub struct Tile(u16);
    impl Debug;
    u16, _owner, _set_owner: 11, 0;           // 12 bits for owner ID (up to 4096 players)
    pub u8, terrain_diff, set_terrain_diff: 14, 12; // 3 bits for terrain type (8 types)
    pub has_fallout, set_fallout: 15;           // 1 bit for fallout flag
}

impl Tile {
    pub fn new(owner: PlayerId, terrain_difficulty: f32) -> Self {
        let mut tile = Tile(0);
        tile._set_owner(owner.0);
        // Map terrain difficulty to 0-7 range
        let terrain_type = ((terrain_difficulty.clamp(0.5, 2.0) - 0.5) / 1.5 * 7.0) as u8;
        tile.set_terrain_diff(terrain_type);
        tile.set_fallout(false);
        tile
    }

    pub fn terrain_difficulty(&self) -> f32 {
        // Map 0-7 range back to terrain difficulty
        0.5 + (f32::from(self.terrain_diff()) / 7.0) * 1.5
    }

    pub fn owner(&self) -> PlayerId {
        PlayerId(self._owner())
    }

    pub fn set_owner(&mut self, owner: PlayerId) {
        self._set_owner(owner.0);
    }
}

/// Represents a player in the game.
#[derive(Debug, Clone, Component)]
pub struct PlayerData {
    pub id: PlayerId,
    pub char: char,
    pub troops: u32,
    pub tile_count: usize,
    /// Sum of x coordinates of all owned tiles (for center calculation)
    pub sum_x: u64,
    /// Sum of y coordinates of all owned tiles (for center calculation)
    pub sum_y: u64,
    pub color: Color,
}

/// Marker component for alive players
#[derive(Component)]
pub struct Alive;

/// Component linking player info text to player entity
#[derive(Component)]
pub struct PlayerInfoText {
    pub player_entity: Entity,
}

/// Resource tracking all active expansion fronts
/// Uses a triangular array where index = X*N + (Y-X-1) for pair (X,Y) where X < Y
/// Positive values mean X is pushing into Y, negative means Y is pushing into X
#[derive(Resource)]
pub struct ActiveExpansions {
    pub fronts: Vec<i32>,
}

impl Default for ActiveExpansions {
    fn default() -> Self {
        Self {
            fronts: vec![0; NUM_PAIRS as usize],
        }
    }
}

impl ActiveExpansions {
    /// Calculate array index for a pair of players
    /// Formula: N*x - (x*(x+1))/2 + y - x - 1 where X < Y
    pub fn pair_index(a: PlayerId, b: PlayerId) -> usize {
        let (x, y) = if a < b { (a.0, b.0) } else { (b.0, a.0) };
        (NUM_ENTITIES * x - (x * (x + 1)) / 2 + y - x - 1) as usize
    }

    /// Add troops to a border, canceling out opposing forces
    pub fn add_troops(&mut self, attacker: PlayerId, defender: PlayerId, troops: i32) {
        let idx = Self::pair_index(attacker, defender);
        let multiplier = if attacker < defender { 1 } else { -1 };
        self.fronts[idx] += troops * multiplier;
    }

    /// Get net troops for a border. If positive, a has troops attacking b, otherwise b has troops attacking a
    pub fn get_net_troops(&self, a: PlayerId, b: PlayerId) -> i32 {
        let idx = Self::pair_index(a, b);
        if a < b {
            self.fronts[idx]
        } else {
            -self.fronts[idx]
        }
    }

    /// Clear a specific border
    pub fn clear_border(&mut self, a: PlayerId, b: PlayerId) {
        let idx = Self::pair_index(a, b);
        self.fronts[idx] = 0;
    }

    /// Remove all borders involving a specific player
    pub fn remove_player(&mut self, player_id: PlayerId) {
        for other_id in 0..NUM_ENTITIES {
            let other_id = PlayerId(other_id);
            if other_id != player_id {
                self.clear_border(player_id, other_id);
            }
        }
    }
}

/// Resource to map [`PlayerId`] to [`Color`] for fast lookups
#[derive(Resource)]
pub struct PlayerColorMap(pub Vec<Color>);

/// Resource to map [`PlayerId`] to [`Entity`] for O(1) lookups
#[derive(Resource)]
pub struct PlayerEntityMap(pub Vec<Option<Entity>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_net_troops_basic() {
        let mut expansions = ActiveExpansions {
            fronts: vec![0; NUM_PAIRS as usize],
        };
        let a = PlayerId::new_unchecked(1);
        let b = PlayerId::new_unchecked(2);

        // Initially, no troops
        assert_eq!(expansions.get_net_troops(a, b), 0);
        assert_eq!(expansions.get_net_troops(b, a), 0);

        // Add troops from a attacking b
        expansions.add_troops(a, b, 5);
        assert_eq!(expansions.get_net_troops(a, b), 5);
        assert_eq!(expansions.get_net_troops(b, a), -5);

        // Add troops from b attacking a (should subtract)
        expansions.add_troops(b, a, 3);
        assert_eq!(expansions.get_net_troops(a, b), 2);
        assert_eq!(expansions.get_net_troops(b, a), -2);

        // Add more troops from a attacking b
        expansions.add_troops(a, b, 7);
        assert_eq!(expansions.get_net_troops(a, b), 9);
        assert_eq!(expansions.get_net_troops(b, a), -9);

        // Clear border
        expansions.clear_border(a, b);
        assert_eq!(expansions.get_net_troops(a, b), 0);
        assert_eq!(expansions.get_net_troops(b, a), 0);
    }

    #[test]
    fn test_get_net_troops_symmetry() {
        let mut expansions = ActiveExpansions {
            fronts: vec![0; NUM_PAIRS as usize],
        };
        let a = PlayerId::new_unchecked(5);
        let b = PlayerId::new_unchecked(3);

        expansions.add_troops(a, b, 10);
        assert_eq!(expansions.get_net_troops(a, b), 10);
        assert_eq!(expansions.get_net_troops(b, a), -10);

        expansions.add_troops(b, a, 4);
        assert_eq!(expansions.get_net_troops(a, b), 6);
        assert_eq!(expansions.get_net_troops(b, a), -6);
    }

    #[test]
    fn test_get_net_troops_multiple_pairs() {
        let mut expansions = ActiveExpansions {
            fronts: vec![0; NUM_PAIRS as usize],
        };
        let a = PlayerId::new_unchecked(0);
        let b = PlayerId::new_unchecked(1);
        let c = PlayerId::new_unchecked(2);

        expansions.add_troops(a, b, 3);
        expansions.add_troops(a, c, 7);
        expansions.add_troops(b, c, 2);

        assert_eq!(expansions.get_net_troops(a, b), 3);
        assert_eq!(expansions.get_net_troops(b, a), -3);
        assert_eq!(expansions.get_net_troops(a, c), 7);
        assert_eq!(expansions.get_net_troops(c, a), -7);
        assert_eq!(expansions.get_net_troops(b, c), 2);
        assert_eq!(expansions.get_net_troops(c, b), -2);
    }
}
