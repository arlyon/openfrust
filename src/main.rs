use bevy::prelude::*;
use bevy::window::WindowResolution;
use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};
use std::collections::HashSet;

// --- GAME CONSTANTS ---
const BOARD_WIDTH: usize = 300;
const BOARD_HEIGHT: usize = 300;
const NUM_PLAYERS: usize = 5;
const TILE_SIZE: f32 = 4.0;
const TROOPS_PER_TILE_INCREASE: f32 = 0.5;

// --- DATA STRUCTURES ---

type PlayerId = usize;
const NO_OWNER: PlayerId = 0;

/// Represents a single tile on the game board.
#[derive(Clone, Copy, Debug)]
struct Tile {
    owner: PlayerId,
}

/// Represents a player in the game.
#[derive(Debug, Clone)]
struct PlayerData {
    id: PlayerId,
    char: char,
    troops: u32,
    border_tiles: HashSet<(usize, usize)>,
    color: Color,
}

/// Resource holding the game board
#[derive(Resource)]
struct Board {
    tiles: Vec<Vec<Tile>>,
}

/// Resource holding all players
#[derive(Resource)]
struct Players {
    list: Vec<PlayerData>,
}

/// Component for tile entities
#[derive(Component)]
struct TileEntity {
    x: usize,
    y: usize,
}

/// Timer for game updates
#[derive(Resource)]
struct GameUpdateTimer(Timer);

/// Startup system to initialize the game
fn setup(mut commands: Commands) {
    // Spawn camera
    commands.spawn(Camera2d);

    let mut rng = rand::rng();
    let board = vec![vec![Tile { owner: NO_OWNER }; BOARD_WIDTH]; BOARD_HEIGHT];

    let player_chars = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
    let player_colors = [
        Color::srgb(1.0, 0.0, 0.0), // Red
        Color::srgb(0.0, 0.0, 1.0), // Blue
        Color::srgb(0.0, 1.0, 0.0), // Green
        Color::srgb(1.0, 1.0, 0.0), // Yellow
        Color::srgb(1.0, 0.0, 1.0), // Magenta
        Color::srgb(0.0, 1.0, 1.0), // Cyan
        Color::srgb(1.0, 0.5, 0.0), // Orange
        Color::srgb(0.5, 0.0, 1.0), // Purple
    ];

    let players = (1..=NUM_PLAYERS)
        .map(|i| PlayerData {
            id: i,
            char: player_chars[i - 1],
            troops: 1000,
            border_tiles: HashSet::new(),
            color: player_colors[i - 1],
        })
        .collect();

    let mut board_res = Board { tiles: board };
    let mut players_res = Players { list: players };

    // Assign starting positions for each player
    for player in players_res.list.clone() {
        loop {
            let x = rng.random_range(10..BOARD_WIDTH - 10);
            let y = rng.random_range(5..BOARD_HEIGHT - 5);
            if board_res.tiles[y][x].owner == NO_OWNER {
                board_res.tiles[y][x].owner = player.id;
                break;
            }
        }
    }

    recalculate_all_borders(&mut board_res, &mut players_res);

    // Spawn tile entities
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let owner = board_res.tiles[y][x].owner;
            let color = if owner == NO_OWNER {
                Color::srgb(0.1, 0.1, 0.1)
            } else {
                players_res.list[owner - 1].color
            };

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
    commands.insert_resource(players_res);
    commands.insert_resource(GameUpdateTimer(Timer::from_seconds(
        0.1,
        TimerMode::Repeating,
    )));
}

/// Game update system
fn update_game(
    time: Res<Time>,
    mut timer: ResMut<GameUpdateTimer>,
    mut board: ResMut<Board>,
    mut players: ResMut<Players>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let mut rng = rand::rng();

    for i in 0..players.list.len() {
        let player_id = players.list[i].id;

        // 1. Gain troops based on number of tiles owned.
        let tiles_owned = count_tiles(&board, player_id);
        let new_troops = ((tiles_owned as f32 * TROOPS_PER_TILE_INCREASE) as u32).max(1);
        let old_troops = players.list[i].troops;
        players.list[i].troops += new_troops;

        bevy::log::info!(
            "updating {}[{}] from {} by {} to {}",
            player_id,
            tiles_owned,
            old_troops,
            new_troops,
            players.list[i].troops
        );

        // 2. AI: Expand into empty space first, then attack enemies.
        if players.list[i].troops > 50 {
            if !attempt_expansion(&mut board, &mut players, player_id, &mut rng) {
                // If no empty space to expand into, attack enemies
                if players.list[i].troops > 500 && rng.random_bool(0.25) {
                    attempt_attack(&mut board, &mut players, player_id, &mut rng);
                }
            }
        }
    }

    recalculate_all_borders(&mut board, &mut players);
}

/// Render system to update tile colors
fn update_tiles(
    board: Res<Board>,
    players: Res<Players>,
    mut query: Query<(&TileEntity, &mut Sprite)>,
) {
    if !board.is_changed() {
        return;
    }

    for (tile_entity, mut sprite) in query.iter_mut() {
        let owner = board.tiles[tile_entity.y][tile_entity.x].owner;
        sprite.color = if owner == NO_OWNER {
            Color::srgb(0.1, 0.1, 0.1)
        } else {
            players.list[owner - 1].color
        };
    }
}

/// Display stats in console
fn display_stats(board: Res<Board>, players: Res<Players>) {
    if !board.is_changed() {
        return;
    }

    println!("--- Stats ---");
    for player in &players.list {
        println!(
            "Player {}: {} Tiles, {} Troops",
            player.char,
            count_tiles(&board, player.id),
            player.troops
        );
    }
}

// --- HELPER FUNCTIONS ---

/// AI attempts to expand into empty (unclaimed) territory.
/// Returns true if expansion was attempted, false if no empty neighbors exist.
fn attempt_expansion(
    board: &mut Board,
    players: &mut Players,
    player_id: PlayerId,
    rng: &mut rand::rngs::ThreadRng,
) -> bool {
    let player = &players.list[player_id - 1];
    if player.border_tiles.is_empty() || player.troops < 10 {
        return false;
    }

    // Find all empty neighbors from border tiles
    let mut empty_neighbors = Vec::new();
    for &(bx, by) in &player.border_tiles {
        for (nx, ny) in get_neighbors(bx, by) {
            if board.tiles[ny][nx].owner == NO_OWNER {
                empty_neighbors.push((nx, ny));
            }
        }
    }

    if empty_neighbors.is_empty() {
        return false;
    }

    // Pick a random empty tile and claim it
    if let Some(&(target_x, target_y)) = empty_neighbors.choose(rng) {
        let expansion_cost = 10;
        players.list[player_id - 1].troops -= expansion_cost;
        board.tiles[target_y][target_x].owner = player_id;
        return true;
    }

    false
}

fn attempt_attack(
    board: &mut Board,
    players: &mut Players,
    attacker_id: PlayerId,
    rng: &mut rand::rngs::ThreadRng,
) {
    let attacker = &players.list[attacker_id - 1];
    if attacker.border_tiles.is_empty() {
        return;
    }

    let border_coords: Vec<&(usize, usize)> = attacker.border_tiles.iter().collect();
    let &(start_x, start_y) = border_coords.choose(rng).unwrap();

    let neighbors = get_neighbors(*start_x, *start_y);
    let enemy_neighbors: Vec<(usize, usize)> = neighbors
        .into_iter()
        .filter(|&(nx, ny)| {
            board.tiles[ny][nx].owner != attacker_id && board.tiles[ny][nx].owner != NO_OWNER
        })
        .collect();

    if let Some(&(target_x, target_y)) = enemy_neighbors.choose(rng) {
        let defender_id = board.tiles[target_y][target_x].owner;

        let attack_force = players.list[attacker_id - 1].troops / 2;
        players.list[attacker_id - 1].troops -= attack_force;

        if attack_force > 100 {
            board.tiles[target_y][target_x].owner = attacker_id;
            if defender_id != NO_OWNER {
                players.list[defender_id - 1].troops += 50;
            }
        }
    }
}

fn count_tiles(board: &Board, player_id: PlayerId) -> usize {
    board
        .tiles
        .iter()
        .flatten()
        .filter(|tile| tile.owner == player_id)
        .count()
}

fn get_neighbors(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut neighbors = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx >= 0 && nx < BOARD_WIDTH as isize && ny >= 0 && ny < BOARD_HEIGHT as isize {
                neighbors.push((nx as usize, ny as usize));
            }
        }
    }
    neighbors
}

fn recalculate_all_borders(board: &mut Board, players: &mut Players) {
    for player in &mut players.list {
        player.border_tiles.clear();
    }

    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let owner_id = board.tiles[y][x].owner;
            if owner_id != NO_OWNER {
                let is_border = get_neighbors(x, y)
                    .iter()
                    .any(|&(nx, ny)| board.tiles[ny][nx].owner != owner_id);
                if is_border {
                    players.list[owner_id - 1].border_tiles.insert((x, y));
                }
            }
        }
    }
}

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
        .add_systems(Startup, setup)
        .add_systems(Update, (update_game, update_tiles))
        .run();
}
