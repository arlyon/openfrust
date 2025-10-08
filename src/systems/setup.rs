use std::collections::HashSet;

use bevy::prelude::*;
use bevy_pancam::PanCam;
use iyes_perf_ui::prelude::PerfUiDefaultEntries;
use rand::Rng;

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, NUM_PLAYERS, TILE_SIZE};

/// Startup system to initialize the game
#[tracing::instrument(skip_all)]
pub fn setup(mut commands: Commands) {
    // Spawn camera with PanCam controls
    commands.spawn((Camera2d, PanCam::default()));
    commands.spawn(PerfUiDefaultEntries::default());

    let mut rng = rand::rng();
    let mut board_res = Board::new(BOARD_WIDTH, BOARD_HEIGHT);

    // Initialize PlayerColorMap and PlayerEntityMap
    let mut player_colors = vec![Color::srgb(0.1, 0.1, 0.1); NUM_PLAYERS + 1];
    player_colors[NO_OWNER] = Color::srgb(0.1, 0.1, 0.1); // Color for wilderness

    let mut player_entity_map = vec![None; NUM_PLAYERS + 1];

    // Spawn player entities and assign starting positions
    for i in 1..=NUM_PLAYERS {
        // Generate random color for each player
        let color = Color::srgb(
            rng.random::<f32>(),
            rng.random::<f32>(),
            rng.random::<f32>(),
        );

        // Find starting position first
        let (start_x, start_y) = loop {
            let x = rng.random_range(10..BOARD_WIDTH - 10);
            let y = rng.random_range(5..BOARD_HEIGHT - 5);
            if board_res.get(x, y).owner() as usize == NO_OWNER {
                break (x, y);
            }
        };

        let player_data = PlayerData {
            id: i,
            char: ((i % 26) as u8 + b'A') as char,
            troops: 1000,
            tile_count: 1,         // Each player starts with one tile
            sum_x: start_x as u64, // Initialize with starting position
            sum_y: start_y as u64, // Initialize with starting position
            border_tiles: HashSet::new(),
            color,
        };

        player_colors[i] = color; // Populate the color map
        board_res.get_mut(start_x, start_y).set_owner(player_data.id as u16);

        let player_entity = commands.spawn((player_data.clone(), Alive)).id();
        player_entity_map[i] = Some(player_entity); // Populate the entity map

        // Spawn player info text
        commands.spawn((
            Text2d::new(format!("P{}: {}", player_data.id, player_data.troops)),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Transform::from_xyz(0.0, 0.0, 10.0),
            PlayerInfoText { player_entity },
        ));
    }

    commands.insert_resource(board_res);
    commands.insert_resource(PlayerColorMap(player_colors));
    commands.insert_resource(PlayerEntityMap(player_entity_map));
    commands.insert_resource(ActiveExpansions::default());
}
