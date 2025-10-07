use bevy::prelude::*;
use bevy::window::WindowResolution;

mod systems;
mod types;
mod utils;

// Re-export types for convenience
pub use types::*;

// --- GAME CONSTANTS ---
pub const BOARD_WIDTH: usize = 1000;
pub const BOARD_HEIGHT: usize = 1000;
pub const NUM_PLAYERS: usize = 1000;
pub const TILE_SIZE: f32 = 2.0;
pub const EXPANSION_RATE_BASE: f32 = 1.0; // Base rate of expansion per troop per tick
pub const NUM_ENTITIES: usize = NUM_PLAYERS + 1;
pub const NUM_PAIRS: usize = (NUM_ENTITIES * (NUM_ENTITIES - 1)) / 2;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "OpenFrust - Bevy Edition".to_string(),
                resolution: WindowResolution::new(800, 800),
                ..default()
            }),
            ..default()
        }))
        .add_message::<TileChangeMessage>()
        .add_systems(
            Startup,
            (systems::setup, systems::initial_border_calculation).chain(),
        )
        .add_systems(
            Update,
            (
                systems::update_game,
                systems::update_tiles,
                systems::update_player_info,
            ),
        )
        .run();
}
