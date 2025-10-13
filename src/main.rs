#![warn(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::needless_pass_by_value)]

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{RenderCreation, WgpuFeatures, WgpuSettings};
use bevy::sprite_render::Material2dPlugin;
use bevy::window::WindowResolution;
use bevy_app_compute::prelude::*;
use bevy_pancam::PanCamPlugin;

pub mod map;
mod shaders;
mod sim_manager;
mod systems;
mod types;

use iyes_perf_ui::{PerfUiAppExt, PerfUiPlugin};
// Re-export types for convenience
pub use types::*;

use crate::sim_manager::SimManager;

// --- GAME CONSTANTS ---
pub const NUM_PLAYERS: u16 = 350; // limit is u11 - 1 ie 2047
pub const EXPANSION_RATE_BASE: f32 = 1.0; // Base rate of expansion per troop per tick
pub const TILE_SIZE: f32 = 1.0;
pub const NUM_ENTITIES: u16 = NUM_PLAYERS + 1;
pub const NUM_PAIRS: u32 = (NUM_ENTITIES as u32 * (NUM_ENTITIES as u32 - 1)) / 2;
pub const ADJACENCY_MATRIX_SIZE: u32 = NUM_ENTITIES as u32 * NUM_ENTITIES as u32;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::from(WgpuSettings {
                        // Request the feature needed for wgpu-profiler
                        features: WgpuFeatures::TIMESTAMP_QUERY,
                        ..Default::default()
                    }),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "OpenFrust - Bevy Edition (GPU)".to_string(),
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
            Material2dPlugin::<shaders::BorderMaterial>::default(),
            // GPU compute plugins
            AppComputePlugin,
        ))
        .add_perf_ui_simple_entry::<systems::PerfUiEntryGpuTime>()
        .insert_resource(Time::<Fixed>::from_hz(10.0))
        .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
        // Load the game map BEFORE initializing the GPU worker (which needs dimensions)
        .insert_resource(map::GameMap::load("giantworldmap").expect("Failed to load map"))
        // Initialize SimManager (owns frame manager and timing)
        .insert_resource(SimManager::default())
        // Initialize RenderSettings with default values (both features enabled)
        .insert_resource(RenderSettings::default())
        // Initialize the sim plugins, gpu-based expansion and cursor querying
        .add_plugins(shaders::compute::ExpansionPlugin)
        .add_plugins(shaders::compute::CursorQueryPlugin)
        .add_plugins(systems::plugins::PlayerInfoPanelPlugin)
        .add_systems(
            Startup,
            (
                systems::setup,
                systems::setup_map_texture,
                systems::setup_gpu_perf_ui,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, sim)
        .add_systems(
            Update,
            (
                systems::update_player_info,
                systems::handle_render_input,
                systems::close_on_esc,
                systems::sync_render_settings_to_materials,
                systems::update_water_animation_time,
            ),
        )
        .run();
}

fn sim(
    mut sim_manager: ResMut<SimManager>,
    mut worker: ResMut<AppComputeWorker<shaders::compute::ExpansionWorker>>,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
    player_map: Res<PlayerEntityMap>,
    time: Res<Time>,
) {
    sim_manager.tick(
        &mut players,
        &mut expansions,
        &mut commands,
        &mut worker,
        &text_query,
        &player_map,
        &time,
    );
}
