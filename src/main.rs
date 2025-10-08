use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_pancam::PanCamPlugin;

mod systems;
mod types;
mod utils;

use iyes_perf_ui::PerfUiPlugin;
// Re-export types for convenience
pub use types::*;

// --- GAME CONSTANTS ---
pub const BOARD_WIDTH: usize = 4096;
pub const BOARD_HEIGHT: usize = 2048;
pub const NUM_PLAYERS: usize = 1000;
pub const EXPANSION_RATE_BASE: f32 = 1.0; // Base rate of expansion per troop per tick
pub const TILE_SIZE: f32 = 1.0;
pub const NUM_ENTITIES: usize = NUM_PLAYERS + 1;
pub const NUM_PAIRS: usize = (NUM_ENTITIES * (NUM_ENTITIES - 1)) / 2;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "OpenFrust - Bevy Edition".to_string(),
                        resolution: WindowResolution::new(800, 800),
                        canvas: Some("#bevy-canvas".to_string()),
                        fit_canvas_to_parent: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            FrameTimeDiagnosticsPlugin::default(),
            PanCamPlugin,
            PerfUiPlugin,
        ))
        .add_message::<TileChangeMessage>()
        .insert_resource(Time::<Fixed>::from_hz(10.0))
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
        .add_systems(
            Startup,
            (
                systems::setup,
                systems::initial_border_calculation,
                systems::setup_map_texture,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, systems::update_game)
        .add_systems(
            Update,
            (systems::update_map_texture, systems::update_player_info),
        )
        .run();
}
