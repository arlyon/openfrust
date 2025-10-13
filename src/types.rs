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
    pub front_lookup: Vec<i32>,
}

impl Default for ActiveExpansions {
    fn default() -> Self {
        Self {
            front_lookup: vec![0i32; NUM_PAIRS as usize],
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

    pub fn index_pair(&self, idx: usize) -> (PlayerId, PlayerId) {
        let n = NUM_ENTITIES as f64;
        let idx_f = idx as f64;

        // Derived from solving the quadratic equation for x
        let x_f = (n - 0.5 - ((n - 0.5).powi(2) - 2.0 * idx_f).sqrt()).floor();
        let x = x_f as u16;

        let pairs_before_x = NUM_ENTITIES * x - (x * (x + 1)) / 2;
        let y = (idx - pairs_before_x as usize) as u16 + x + 1;

        let (x, y) = if self.front_lookup[idx] >= 0 {
            (x, y)
        } else {
            (y, x)
        };

        (PlayerId::new_unchecked(x), PlayerId::new_unchecked(y))
    }

    /// Add troops to a border, canceling out opposing forces
    pub fn add_troops(&mut self, attacker: PlayerId, defender: PlayerId, troops: i32) {
        let idx = Self::pair_index(attacker, defender);
        let multiplier = if attacker < defender { 1 } else { -1 };
        self.front_lookup[idx] += troops * multiplier;
    }

    /// Get net troops for a border. If positive, a has troops attacking b, otherwise b has troops attacking a
    pub fn get_net_troops(&self, a: PlayerId, b: PlayerId) -> i32 {
        let idx = Self::pair_index(a, b);
        if a < b {
            self.front_lookup[idx]
        } else {
            -self.front_lookup[idx]
        }
    }

    /// Clear a specific border
    pub fn clear_border(&mut self, a: PlayerId, b: PlayerId) {
        let idx = Self::pair_index(a, b);
        self.front_lookup[idx] = 0;
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

    pub fn print(&self) -> String {
        self.front_lookup
            .iter()
            .enumerate()
            .filter(|(_, troops)| **troops != 0)
            .map(|(idx, troops)| (self.index_pair(idx), troops))
            .map(|((a, b), troops)| format!("- {} -> {} ({} troops)", a, b, troops.abs()))
            .collect::<Vec<String>>()
            .join("\n")
    }
}

/// Resource to map [`PlayerId`] to [`Color`] for fast lookups
#[derive(Resource)]
pub struct PlayerColorMap(pub Vec<Color>);

/// Resource to map [`PlayerId`] to [`Entity`] for O(1) lookups
#[derive(Resource)]
pub struct PlayerEntityMap(pub Vec<Option<Entity>>);

/// Resource to control rendering features
#[derive(Resource, Clone)]
pub struct RenderSettings {
    /// Enable/disable fancy animated water rendering
    pub enable_water_animation: bool,
    /// Enable/disable player rendering (territory colors and borders)
    pub enable_players: bool,
    pub enable_sphere_projection: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            enable_water_animation: true,
            enable_players: true,
            enable_sphere_projection: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn test_get_net_troops_basic() {
        let mut expansions = ActiveExpansions {
            front_lookup: vec![0; NUM_PAIRS as usize],
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
            front_lookup: vec![0; NUM_PAIRS as usize],
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
            front_lookup: vec![0; NUM_PAIRS as usize],
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

    #[test_case(0, 1, 0; "first pair (0,1)")]
    #[test_case(1, 0, 0; "first pair reversed (1,0)")]
    #[test_case(0, 2, 1; "second pair (0,2)")]
    #[test_case(2, 0, 1; "second pair reversed (2,0)")]
    #[test_case(0, 3, 2; "third pair (0,3)")]
    #[test_case(3, 0, 2; "third pair reversed (3,0)")]
    #[test_case(1, 2, NUM_ENTITIES as usize - 1; "pair (1,2)")]
    #[test_case(2, 1, NUM_ENTITIES as usize - 1; "pair (2,1) reversed")]
    fn test_pair_index_basic(player_a: u16, player_b: u16, expected_idx: usize) {
        let a = PlayerId::new_unchecked(player_a);
        let b = PlayerId::new_unchecked(player_b);
        assert_eq!(ActiveExpansions::pair_index(a, b), expected_idx);
    }

    #[test]
    fn test_pair_index_symmetry() {
        // Test that pair_index(a, b) == pair_index(b, a) for all pairs
        for i in 0..10 {
            for j in (i + 1)..10 {
                let a = PlayerId::new_unchecked(i);
                let b = PlayerId::new_unchecked(j);
                assert_eq!(
                    ActiveExpansions::pair_index(a, b),
                    ActiveExpansions::pair_index(b, a),
                    "pair_index should be symmetric for ({}, {})",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_pair_index_uniqueness() {
        // Test that each pair gets a unique index
        use std::collections::HashSet;
        let mut seen_indices = HashSet::new();

        for i in 0..20 {
            for j in (i + 1)..20 {
                let a = PlayerId::new_unchecked(i);
                let b = PlayerId::new_unchecked(j);
                let idx = ActiveExpansions::pair_index(a, b);

                assert!(
                    seen_indices.insert(idx),
                    "Index {} already used! Collision for pair ({}, {})",
                    idx,
                    i,
                    j
                );
            }
        }
    }

    #[test_case(0, 0, 1; "index 0 maps to (0,1)")]
    #[test_case(1, 0, 2; "index 1 maps to (0,2)")]
    #[test_case(2, 0, 3; "index 2 maps to (0,3)")]
    #[test_case(NUM_ENTITIES as usize - 1, 1, 2; "index N-1 maps to (1,2)")]
    fn test_index_pair_basic(idx: usize, expected_a: u16, expected_b: u16) {
        let expansions = ActiveExpansions::default();
        let (a, b) = expansions.index_pair(idx);
        assert_eq!(a.0, expected_a);
        assert_eq!(b.0, expected_b);
    }

    #[test_case(0, 1; "pair (0,1)")]
    #[test_case(0, 5; "pair (0,5)")]
    #[test_case(0, 10; "pair (0,10)")]
    #[test_case(1, 2; "pair (1,2)")]
    #[test_case(1, 5; "pair (1,5)")]
    #[test_case(5, 10; "pair (5,10)")]
    #[test_case(10, 20; "pair (10,20)")]
    #[test_case(15, 29; "pair (15,29)")]
    fn test_index_pair_roundtrip(player_a: u16, player_b: u16) {
        // Test that pair_index and index_pair are inverse operations
        let expansions = ActiveExpansions::default();

        let a = PlayerId::new_unchecked(player_a);
        let b = PlayerId::new_unchecked(player_b);

        let idx = ActiveExpansions::pair_index(a, b);
        let (recovered_a, recovered_b) = expansions.index_pair(idx);

        assert_eq!(
            recovered_a.0, player_a,
            "Failed to recover first player for pair ({}, {}), index {}",
            player_a, player_b, idx
        );
        assert_eq!(
            recovered_b.0, player_b,
            "Failed to recover second player for pair ({}, {}), index {}",
            player_a, player_b, idx
        );
    }

    #[test]
    fn test_index_pair_all_indices() {
        // Test that index_pair works for a range of indices
        let expansions = ActiveExpansions::default();

        for idx in 0..100 {
            let (a, b) = expansions.index_pair(idx);

            // Verify that a < b (by construction of the triangular array)
            assert!(
                a < b,
                "Expected a < b, got a={}, b={} for index {}",
                a,
                b,
                idx
            );

            // Verify roundtrip
            let recovered_idx = ActiveExpansions::pair_index(a, b);
            assert_eq!(
                recovered_idx, idx,
                "Roundtrip failed: index {} -> ({}, {}) -> index {}",
                idx, a, b, recovered_idx
            );
        }
    }

    #[test_case(NUM_ENTITIES - 2, NUM_ENTITIES - 1; "max and second max")]
    #[test_case(0, NUM_ENTITIES - 1; "min and max")]
    #[test_case(0, 1; "min pair")]
    fn test_pair_index_edge_cases(player_a: u16, player_b: u16) {
        // Test with edge case player IDs - should not panic
        let a = PlayerId::new_unchecked(player_a);
        let b = PlayerId::new_unchecked(player_b);
        let _idx = ActiveExpansions::pair_index(a, b);
    }
}
