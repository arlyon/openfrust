use bevy::prelude::*;
use bevy_pancam::PanCam;
use iyes_perf_ui::prelude::PerfUiDefaultEntries;
use rand::Rng;

use crate::map::GameMap;
use crate::NUM_PLAYERS;
use crate::{
    NUM_ENTITIES,
    types::{
        ActiveExpansions, Alive, NO_OWNER, PlayerColorMap, PlayerData, PlayerEntityMap, PlayerId,
        PlayerInfoText,
    },
};

const WILDERNESS_COLOR: Color = Color::srgb(0.74, 0.8, 0.53);

/// Startup system to initialize the game, creating the camera, players,
/// and expansions.
#[tracing::instrument(skip_all)]
pub fn setup(mut commands: Commands, map: Res<GameMap>) {
    let board_width = map.width() as f32;
    let board_height = map.height() as f32;

    // Spawn camera with PanCam controls
    commands.spawn((
        Camera2d,
        PanCam {
            min_scale: 1.0 / 16.0,
            max_scale: 2.0,
            speed: 2.0,
            max_x: board_width / 2.0 + 100.0,
            max_y: board_height / 2.0 + 100.0,
            min_x: -board_width / 2.0 - 100.0,
            min_y: -board_height / 2.0 - 100.0,
            ..Default::default()
        },
    ));
    commands.spawn(PerfUiDefaultEntries::default());

    let mut rng = rand::rng();

    // Initialize PlayerColorMap and PlayerEntityMap
    let mut player_colors = vec![Color::srgb(0.1, 0.1, 0.1); NUM_ENTITIES.into()];
    player_colors[usize::from(NO_OWNER)] = WILDERNESS_COLOR;

    let mut player_entity_map = vec![None; NUM_ENTITIES.into()];

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
            let x = rng.random_range(10..board_width as usize - 10);
            let y = rng.random_range(5..board_height as usize - 5);
            break (x, y);
        };

        let player_data = PlayerData {
            id: PlayerId::new(i),
            char: ((i % 26) as u8 + b'A') as char,
            troops: 1000,
            tile_count: 1,         // Each player starts with one tile
            sum_x: start_x as u64, // Initialize with starting position
            sum_y: start_y as u64, // Initialize with starting position
            color,
        };

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
}
