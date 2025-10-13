use bevy::prelude::*;
use bevy_app_compute::prelude::AppComputeWorker;
use bevy_pancam::PanCam;
use iyes_perf_ui::prelude::PerfUiDefaultEntries;
use rand::Rng;

use crate::map::GameMap;
use crate::shaders::compute::ExpansionWorker;
use crate::{
    NUM_ENTITIES,
    types::{
        ActiveExpansions, Alive, NO_OWNER, PlayerColorMap, PlayerData, PlayerEntityMap, PlayerId,
        PlayerInfoText,
    },
};
use crate::{NUM_PLAYERS, Tile};

const WILDERNESS_COLOR: Color = Color::srgb(0.74, 0.8, 0.53);

/// Startup system to initialize the game, creating the camera, players,
/// and expansions.
#[tracing::instrument(skip_all)]
pub fn setup(
    mut commands: Commands,
    map: Res<GameMap>,
    mut worker: ResMut<AppComputeWorker<ExpansionWorker>>,
) {
    let board_width = map.width();
    let board_height = map.height();

    // Spawn camera with PanCam controls
    commands.spawn((
        Camera2d,
        PanCam {
            min_scale: 1.0 / 16.0,
            max_scale: 2.0,
            speed: 2.0,
            max_x: board_width as f32 / 2.0 + 100.0,
            max_y: board_height as f32 / 2.0 + 100.0,
            min_x: -(board_width as f32) / 2.0 - 100.0,
            min_y: -(board_height as f32) / 2.0 - 100.0,
            ..Default::default()
        },
    ));
    commands.spawn(PerfUiDefaultEntries::default());

    let mut rng = rand::rng();

    // Initialize PlayerColorMap and PlayerEntityMap
    let mut player_colors = vec![Color::srgb(0.1, 0.1, 0.1); NUM_ENTITIES.into()];
    player_colors[usize::from(NO_OWNER)] = WILDERNESS_COLOR;

    let mut player_entity_map = vec![None; NUM_ENTITIES.into()];

    // 1. Create a full board representation on the CPU, initialized to wilderness.
    // The `Tile` struct is a u16 bitfield, matching the data layout we need.
    let mut initial_board: Vec<Tile> =
        vec![Tile::new(NO_OWNER, 1.0); map.width() as usize * map.height() as usize];

    // Spawn player entities and assign starting positions
    for i in 1..=NUM_PLAYERS {
        // Generate random color for each player
        let color = Color::hsl(
            rng.random::<f32>() * 360.0,
            rng.random::<f32>() / 2.0 + 0.5,
            0.65,
        );

        // Find starting position first
        let (start_x, start_y) = loop {
            let x = rng.random_range(10..board_width - 10);
            let y = rng.random_range(5..board_height - 5);
            break (x, y);
        };

        let player_data = PlayerData {
            id: PlayerId::new(i),
            char: ((i % 26) as u8 + b'A') as char,
            troops: 1000,
            tile_count: 1,         // Each player starts with one tile
            sum_x: start_x.into(), // Initialize with starting position
            sum_y: start_y.into(), // Initialize with starting position
            color,
        };

        // Use the map's helper function to get the correct 1D index.
        if let Some(index) = map.tile_ref(start_x, start_y) {
            // Create a tile owned by this player with default terrain difficulty.
            initial_board[index] = Tile::new(player_data.id, 1.0);
        }

        player_colors[usize::from(i)] = color; // Populate the color map

        let player_entity = commands.spawn((player_data.clone(), Alive)).id();
        player_entity_map[usize::from(i)] = Some(player_entity); // Populate the entity map

        // Spawn player info text
        commands.spawn((
            Text2d::new(format!("P{}: {}", player_data.id, player_data.troops)),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 0.0, 10.0),
            PlayerInfoText { player_entity },
        ));
    }

    commands.insert_resource(PlayerColorMap(player_colors));
    commands.insert_resource(PlayerEntityMap(player_entity_map));
    commands.insert_resource(ActiveExpansions::default());

    // 3. Pack the board data. Your shaders expect two `u16` tiles per `u32`.
    let packed_board: Vec<u32> = initial_board
        .chunks(2)
        .map(|chunk| {
            // Get the raw u16 data from the Tile bitfield struct.
            let tile1_data = chunk[0].0 as u32;
            // Handle the case of an odd number of total tiles.
            let tile2_data = if chunk.len() > 1 {
                chunk[1].0 as u32
            } else {
                0 // If there's no second tile, its data is 0.
            };

            // Bit-shift the second tile into the high bits of the u32.
            (tile2_data << 16) | tile1_data
        })
        .collect();

    // 4. Write the final packed data to the GPU's `board_in` buffer.
    // This synchronizes the initial state before the first simulation tick.
    bevy::log::info!("Seeding GPU board with initial player positions...");
    if let Err(e) = worker.try_write_slice("board_in", &packed_board) {
        bevy::log::error!("Failed to write initial board data to GPU: {:?}", e);
    }
}
