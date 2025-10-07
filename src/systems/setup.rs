use std::collections::HashSet;

use bevy::prelude::*;
use rand::Rng;

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, NUM_PLAYERS, TILE_SIZE};

/// Startup system to initialize the game
#[tracing::instrument(skip_all)]
pub fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2d);

    let mut rng = rand::rng();
    let board = vec![
        vec![
            Tile {
                owner: NO_OWNER,
                terrain_difficulty: 1.0,
            };
            BOARD_WIDTH
        ];
        BOARD_HEIGHT
    ];

    let mut board_res = Board { tiles: board };

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
            if board_res.tiles[y][x].owner == NO_OWNER {
                break (x, y);
            }
        };

        let player_data = PlayerData {
            id: i,
            char: ((i % 26) as u8 + b'A') as char,
            troops: 1000,
            tile_count: 1, // Each player starts with one tile
            sum_x: start_x as u64, // Initialize with starting position
            sum_y: start_y as u64, // Initialize with starting position
            border_tiles: HashSet::new(),
            color,
        };

        board_res.tiles[start_y][start_x].owner = player_data.id;

        let player_entity = commands.spawn((player_data.clone(), Alive)).id();

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

    // Spawn tile entities
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let color = Color::srgb(0.1, 0.1, 0.1); // Start with all tiles gray

            let pos_x = (x as f32 - BOARD_WIDTH as f32 / 2.0) * TILE_SIZE;
            let pos_y = (BOARD_HEIGHT as f32 / 2.0 - y as f32) * TILE_SIZE;

            commands.spawn((
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(TILE_SIZE - 1.0, TILE_SIZE - 1.0)),
                    ..default()
                },
                Transform::from_xyz(pos_x, pos_y, 0.0),
                TileEntity { x, y },
            ));
        }
    }

    commands.insert_resource(board_res);
    commands.insert_resource(ActiveExpansions::default());
    commands.insert_resource(GameUpdateTimer(Timer::from_seconds(
        0.1,
        TimerMode::Repeating,
    )));
}
