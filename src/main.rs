use bevy::prelude::*;
use bevy::window::WindowResolution;
use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};
use std::collections::{HashMap, HashSet};

// --- GAME CONSTANTS ---
const BOARD_WIDTH: usize = 300;
const BOARD_HEIGHT: usize = 300;
const NUM_PLAYERS: usize = 5;
const TILE_SIZE: f32 = 4.0;
const TROOPS_PER_TILE_INCREASE: f32 = 0.5;
const EXPANSION_RATE_BASE: f32 = 1.0; // Base rate of expansion per troop per tick

// --- DATA STRUCTURES ---

type PlayerId = usize;
const NO_OWNER: PlayerId = 0;

/// Represents a single tile on the game board.
#[derive(Clone, Copy, Debug)]
struct Tile {
    owner: PlayerId,
    /// Terrain difficulty multiplier (1.0 = normal)
    terrain_difficulty: f32,
}

/// Key for expansion fronts between two players
type ExpansionKey = (PlayerId, PlayerId);

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

/// Resource tracking all active expansion fronts
#[derive(Resource, Default)]
struct ActiveExpansions {
    /// Maps (attacker_id, attackee_id) to number of troops pushing that border
    /// attackee_id = 0 means expanding into empty space
    fronts: HashMap<ExpansionKey, u32>,
}

/// Component for tile entities
#[derive(Component)]
struct TileEntity {
    x: usize,
    y: usize,
}

/// Component for player info text
#[derive(Component)]
struct PlayerInfoText {
    player_id: PlayerId,
}

/// Timer for game updates
#[derive(Resource)]
struct GameUpdateTimer(Timer);

/// Startup system to initialize the game
fn setup(mut commands: Commands) {
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
    commands.insert_resource(ActiveExpansions::default());
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
    mut expansions: ResMut<ActiveExpansions>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let mut rng = rand::rng();

    // 1. Update troop generation and AI decisions
    for i in 0..players.list.len() {
        let player_id = players.list[i].id;

        // Gain troops based on number of tiles owned.
        let tiles_owned = count_tiles(&board, player_id);
        let new_troops = ((tiles_owned as f32 * TROOPS_PER_TILE_INCREASE) as u32).max(1);
        players.list[i].troops += new_troops;

        bevy::log::debug!(
            "Player {} [{}]: {} troops (+{})",
            player_id,
            tiles_owned,
            players.list[i].troops,
            new_troops
        );

        // AI: Assign troops to expansion fronts
        if players.list[i].troops > 50 {
            assign_expansion_troops(&board, &mut players, &mut expansions, player_id);
        }
    }

    // 2. Log active expansion fronts
    if !expansions.fronts.is_empty() {
        bevy::log::info!("Active expansion fronts:");
        for (&(attacker, defender), &troops) in &expansions.fronts {
            let defender_name = if defender == NO_OWNER {
                "Empty".to_string()
            } else {
                format!("Player {}", defender)
            };
            bevy::log::info!(
                "  Player {} -> {}: {} troops",
                attacker,
                defender_name,
                troops
            );
        }
    }

    // 3. Process all expansion fronts and move borders
    process_expansion_fronts(&mut board, &mut players, &mut expansions);

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

/// AI assigns troops to expansion fronts based on border neighbors
fn assign_expansion_troops(
    board: &Board,
    players: &mut Players,
    expansions: &mut ActiveExpansions,
    player_id: PlayerId,
) {
    let player = &players.list[player_id - 1];
    if player.border_tiles.is_empty() || player.troops < 10 {
        return;
    }

    // Count neighbors for each border type
    let mut neighbor_counts: HashMap<PlayerId, usize> = HashMap::new();

    for &(bx, by) in &player.border_tiles {
        for (nx, ny) in get_neighbors(bx, by) {
            let neighbor_owner = board.tiles[ny][nx].owner;
            if neighbor_owner != player_id {
                *neighbor_counts.entry(neighbor_owner).or_insert(0) += 1;
            }
        }
    }

    if neighbor_counts.is_empty() {
        return;
    }

    // Assign half of available troops to expansion fronts proportionally
    let troops_to_assign = player.troops / 2;
    let total_border_length: usize = neighbor_counts.values().sum();

    for (neighbor_id, border_length) in neighbor_counts {
        let proportion = border_length as f32 / total_border_length as f32;
        let troops = (troops_to_assign as f32 * proportion) as u32;

        if troops > 0 {
            let key = (player_id, neighbor_id);
            *expansions.fronts.entry(key).or_insert(0) += troops;
            players.list[player_id - 1].troops -= troops;
        }
    }
}

/// Process all expansion fronts and move borders based on relative troop counts
fn process_expansion_fronts(
    board: &mut Board,
    players: &mut Players,
    expansions: &mut ActiveExpansions,
) {
    // For each pair of players with a shared border, calculate net force
    let mut border_velocities: HashMap<ExpansionKey, i32> = HashMap::new();

    for (&(attacker, defender), &troops) in &expansions.fronts {
        let forward_key = (attacker, defender);
        let reverse_key = (defender, attacker);

        let forward_troops = troops as i32;
        let reverse_troops = expansions.fronts.get(&reverse_key).copied().unwrap_or(0) as i32;

        // Net velocity: positive means attacker is winning
        let net_velocity = forward_troops - reverse_troops;
        border_velocities.insert(forward_key, net_velocity);
    }

    // Apply border movements based on velocities
    for (&(attacker, defender), &velocity) in &border_velocities {
        if velocity <= 0 {
            continue; // Only process winning side once
        }

        // Calculate how many tiles to move based on velocity
        let tiles_to_move = (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as usize;

        // Find all border tiles where attacker touches defender
        let mut contested_tiles = Vec::new();
        if let Some(attacker_data) = players.list.get(attacker - 1) {
            for &(bx, by) in &attacker_data.border_tiles {
                for (nx, ny) in get_neighbors(bx, by) {
                    if board.tiles[ny][nx].owner == defender {
                        contested_tiles.push((nx, ny));
                    }
                }
            }
        }

        // Move tiles evenly along the border
        let tiles_to_claim = tiles_to_move.min(contested_tiles.len());
        for &(x, y) in contested_tiles.iter().take(tiles_to_claim) {
            board.tiles[y][x].owner = attacker;
        }
    }

    // Reduce troop counts for each tick (troops are consumed as they push)
    for (key, troops) in expansions.fronts.iter_mut() {
        let decay_rate = (*troops as f32 * 0.1).max(1.0) as u32; // 10% per tick, minimum 1
        *troops = troops.saturating_sub(decay_rate);
    }

    // Remove fronts with no troops left
    expansions.fronts.retain(|_, &mut troops| troops > 0);
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
