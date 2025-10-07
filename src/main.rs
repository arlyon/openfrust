use bevy::prelude::*;
use bevy::window::WindowResolution;
use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};
use std::collections::{HashMap, HashSet};

// --- GAME CONSTANTS ---
const BOARD_WIDTH: usize = 500;
const BOARD_HEIGHT: usize = 500;
const NUM_PLAYERS: usize = 5;
const TILE_SIZE: f32 = 2.0;
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

/// Total number of possible player pairs including wilderness (NO_OWNER)
const NUM_ENTITIES: usize = NUM_PLAYERS + 1;
const NUM_PAIRS: usize = (NUM_ENTITIES * (NUM_ENTITIES - 1)) / 2;

/// Represents a player in the game.
#[derive(Debug, Clone, Component)]
struct PlayerData {
    id: PlayerId,
    char: char,
    troops: u32,
    border_tiles: HashSet<(usize, usize)>,
    color: Color,
}

/// Marker component for alive players
#[derive(Component)]
struct Alive;

/// Resource holding the game board
#[derive(Resource)]
struct Board {
    tiles: Vec<Vec<Tile>>,
}

/// Component linking player info text to player entity
#[derive(Component)]
struct PlayerInfoText {
    player_entity: Entity,
}

/// Resource tracking all active expansion fronts
/// Uses a triangular array where index = X*N + (Y-X-1) for pair (X,Y) where X < Y
/// Positive values mean X is pushing into Y, negative means Y is pushing into X
#[derive(Resource)]
struct ActiveExpansions {
    fronts: [i32; NUM_PAIRS],
}

impl Default for ActiveExpansions {
    fn default() -> Self {
        Self {
            fronts: [0; NUM_PAIRS],
        }
    }
}

impl ActiveExpansions {
    /// Calculate array index for a pair of players
    /// Formula: N*x - (x*(x+1))/2 + y - x - 1 where X < Y
    fn pair_index(a: PlayerId, b: PlayerId) -> usize {
        let (x, y) = if a < b { (a, b) } else { (b, a) };
        NUM_ENTITIES * x - (x * (x + 1)) / 2 + y - x - 1
    }

    /// Add troops to a border, canceling out opposing forces
    fn add_troops(&mut self, attacker: PlayerId, defender: PlayerId, troops: i32) {
        let idx = Self::pair_index(attacker, defender);
        let multiplier = if attacker < defender { 1 } else { -1 };
        self.fronts[idx] += troops * multiplier;
    }

    /// Get net troops for a border (positive means lower ID is winning)
    fn get_net_troops(&self, a: PlayerId, b: PlayerId) -> i32 {
        let idx = Self::pair_index(a, b);
        self.fronts[idx]
    }

    /// Clear a specific border
    fn clear_border(&mut self, a: PlayerId, b: PlayerId) {
        let idx = Self::pair_index(a, b);
        self.fronts[idx] = 0;
    }

    /// Remove all borders involving a specific player
    fn remove_player(&mut self, player_id: PlayerId) {
        for other_id in 0..NUM_ENTITIES {
            if other_id != player_id {
                self.clear_border(player_id, other_id);
            }
        }
    }
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

    let mut board_res = Board { tiles: board };

    // Spawn player entities and assign starting positions
    for i in 1..=NUM_PLAYERS {
        let player_data = PlayerData {
            id: i,
            char: player_chars[i - 1],
            troops: 1000,
            border_tiles: HashSet::new(),
            color: player_colors[i - 1],
        };

        // Find starting position
        loop {
            let x = rng.random_range(10..BOARD_WIDTH - 10);
            let y = rng.random_range(5..BOARD_HEIGHT - 5);
            if board_res.tiles[y][x].owner == NO_OWNER {
                board_res.tiles[y][x].owner = player_data.id;
                break;
            }
        }

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
            let owner = board_res.tiles[y][x].owner;
            let color = if owner == NO_OWNER {
                Color::srgb(0.1, 0.1, 0.1)
            } else {
                player_colors[owner - 1]
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
    commands.insert_resource(ActiveExpansions::default());
    commands.insert_resource(GameUpdateTimer(Timer::from_seconds(
        0.1,
        TimerMode::Repeating,
    )));
}

/// System to recalculate borders on startup
fn initial_border_calculation(mut players: Query<&mut PlayerData, With<Alive>>, board: Res<Board>) {
    for mut player in players.iter_mut() {
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
                    if let Some(mut player) = players.iter_mut().find(|p| p.id == owner_id) {
                        player.border_tiles.insert((x, y));
                    }
                }
            }
        }
    }
}

/// Game update system
fn update_game(
    time: Res<Time>,
    mut timer: ResMut<GameUpdateTimer>,
    mut board: ResMut<Board>,
    mut players: Query<(Entity, &mut PlayerData), With<Alive>>,
    mut expansions: ResMut<ActiveExpansions>,
    mut commands: Commands,
    text_query: Query<(Entity, &PlayerInfoText)>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    // 1. Check for eliminations and update troop generation
    let mut to_eliminate = Vec::new();

    for (entity, mut player) in players.iter_mut() {
        let tiles_owned = count_tiles(&board, player.id);

        if tiles_owned == 0 {
            bevy::log::warn!("Player {} has been eliminated!", player.id);
            to_eliminate.push((entity, player.id));
            continue;
        }

        // Calculate max troops based on territory (non-linear scaling)
        let max_troops = (2.0 * ((tiles_owned as f32).powf(0.6) * 1000.0 + 50000.0)) as u32;

        // Calculate troop growth with braking mechanism
        let base_growth = 10.0 + (player.troops as f32).powf(0.73) / 4.0;
        let braking_ratio = (1.0 - (player.troops as f32 / max_troops as f32)).max(0.0);
        let new_troops = (base_growth * braking_ratio) as u32;

        player.troops = (player.troops + new_troops).min(max_troops);

        bevy::log::info!(
            "Player {} [{}]: {}/{} troops (+{})",
            player.id,
            tiles_owned,
            player.troops,
            max_troops,
            new_troops
        );

        // AI: Assign troops to expansion fronts
        // TODO: Temporarily disabled player vs player combat
        if player.troops > 50 {
            assign_expansion_troops(&board, &player, &mut expansions);
        }
    }

    // 2. Handle eliminations
    for (entity, player_id) in to_eliminate {
        // Remove Alive marker
        commands.entity(entity).remove::<Alive>();

        // Remove all expansion fronts to/from this player
        expansions.remove_player(player_id);

        // Delete name tag
        for (text_entity, info) in text_query.iter() {
            if info.player_entity == entity {
                commands.entity(text_entity).despawn();
            }
        }
    }

    // 3. Log active expansion fronts
    let has_active_fronts = expansions.fronts.iter().any(|&troops| troops != 0);
    if has_active_fronts {
        bevy::log::info!("Active expansion fronts:");
        for a in 0..NUM_ENTITIES {
            for b in (a + 1)..NUM_ENTITIES {
                let net_troops = expansions.get_net_troops(a, b);
                if net_troops != 0 {
                    let (attacker, defender, troops) = if net_troops > 0 {
                        (a, b, net_troops)
                    } else {
                        (b, a, -net_troops)
                    };
                    let defender_name = if defender == NO_OWNER {
                        "Empty".to_string()
                    } else {
                        format!("Player {}", defender)
                    };
                    let attacker_name = if attacker == NO_OWNER {
                        "Empty".to_string()
                    } else {
                        format!("Player {}", attacker)
                    };
                    bevy::log::info!(
                        "  {} -> {}: {} troops",
                        attacker_name,
                        defender_name,
                        troops
                    );
                }
            }
        }
    }

    // 4. Process all expansion fronts and move borders
    process_expansion_fronts(&mut board, &mut expansions);

    // 5. Recalculate borders
    recalculate_all_borders(&board, &mut players);

    // 6. Clear expansion fronts for pairs that no longer share a border and refund troops
    clear_disconnected_fronts(&board, &mut expansions, &mut players);
}

/// Render system to update tile colors
fn update_tiles(
    board: Res<Board>,
    players: Query<&PlayerData, With<Alive>>,
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
            players
                .iter()
                .find(|p| p.id == owner)
                .map(|p| p.color)
                .unwrap_or(Color::srgb(0.1, 0.1, 0.1))
        };
    }
}

/// Update player info text with troop counts and position at territory center
fn update_player_info(
    board: Res<Board>,
    players: Query<(Entity, &PlayerData), (With<Alive>, Changed<PlayerData>)>,
    mut text_query: Query<(&PlayerInfoText, &mut Text2d, &mut Transform)>,
) {
    if players.is_empty() && !board.is_changed() {
        return;
    }

    for (info, mut text, mut transform) in text_query.iter_mut() {
        if let Some((_, player)) = players.iter().find(|(e, _)| *e == info.player_entity) {
            // Update text
            text.0 = format!("P{}: {}", player.id, player.troops);

            // Calculate center of player's territory
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut count = 0;

            for y in 0..BOARD_HEIGHT {
                for x in 0..BOARD_WIDTH {
                    if board.tiles[y][x].owner == player.id {
                        sum_x += x as f32;
                        sum_y += y as f32;
                        count += 1;
                    }
                }
            }

            if count > 0 {
                let center_x = sum_x / count as f32;
                let center_y = sum_y / count as f32;

                let pos_x = (center_x - BOARD_WIDTH as f32 / 2.0) * TILE_SIZE;
                let pos_y = (BOARD_HEIGHT as f32 / 2.0 - center_y) * TILE_SIZE;

                transform.translation.x = pos_x;
                transform.translation.y = pos_y;
            }
        }
    }
}

// --- HELPER FUNCTIONS ---

/// AI assigns troops to expansion fronts based on border neighbors
fn assign_expansion_troops(board: &Board, player: &PlayerData, expansions: &mut ActiveExpansions) {
    if player.border_tiles.is_empty() || player.troops < 10 {
        return;
    }

    // Count neighbors for each border type
    let mut neighbor_counts: HashMap<PlayerId, usize> = HashMap::new();

    for &(bx, by) in &player.border_tiles {
        for (nx, ny) in get_neighbors(bx, by) {
            let neighbor_owner = board.tiles[ny][nx].owner;
            if neighbor_owner != player.id && neighbor_owner == NO_OWNER {
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
        let troops = (troops_to_assign as f32 * proportion) as i32;

        if troops > 0 {
            expansions.add_troops(player.id, neighbor_id, troops);
        }
    }
}

/// Process all expansion fronts and move borders based on relative troop counts
fn process_expansion_fronts(board: &mut Board, expansions: &mut ActiveExpansions) {
    // Process each border pair
    for a in 0..NUM_ENTITIES {
        for b in (a + 1)..NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);

            if net_troops == 0 {
                continue;
            }

            // Determine attacker and defender based on sign
            let (attacker, defender) = if net_troops > 0 { (a, b) } else { (b, a) };
            let velocity = net_troops.abs();

            // Calculate how many tiles to move based on velocity
            let tiles_to_move = (velocity as f32 * EXPANSION_RATE_BASE / 100.0).max(0.1) as usize;

            // Find all border tiles where attacker can expand
            let mut contested_tiles = Vec::new();
            for y in 0..BOARD_HEIGHT {
                for x in 0..BOARD_WIDTH {
                    if board.tiles[y][x].owner == attacker {
                        for (nx, ny) in get_neighbors(x, y) {
                            if board.tiles[ny][nx].owner == defender {
                                contested_tiles.push((nx, ny));
                            }
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
    }

    // Reduce troop counts for each tick (troops are consumed as they push)
    for troops in expansions.fronts.iter_mut() {
        if *troops != 0 {
            let abs_troops = troops.abs();
            let decay_rate = ((abs_troops as f32 * 0.1).max(1.0) as i32).min(abs_troops);
            *troops = if *troops > 0 {
                troops.saturating_sub(decay_rate)
            } else {
                troops.saturating_add(decay_rate)
            };
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

fn recalculate_all_borders(
    board: &Board,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
) {
    for (_, mut player) in players.iter_mut() {
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
                    if let Some((_, mut player)) =
                        players.iter_mut().find(|(_, p)| p.id == owner_id)
                    {
                        player.border_tiles.insert((x, y));
                    }
                }
            }
        }
    }
}

/// Clear expansion fronts between players that no longer share a border and refund troops
fn clear_disconnected_fronts(
    board: &Board,
    expansions: &mut ActiveExpansions,
    players: &mut Query<(Entity, &mut PlayerData), With<Alive>>,
) {
    for a in 0..NUM_ENTITIES {
        for b in (a + 1)..NUM_ENTITIES {
            let net_troops = expansions.get_net_troops(a, b);
            if net_troops == 0 {
                continue;
            }

            // Check if these two players still share a border
            let mut shares_border = false;
            'outer: for y in 0..BOARD_HEIGHT {
                for x in 0..BOARD_WIDTH {
                    if board.tiles[y][x].owner == a {
                        for (nx, ny) in get_neighbors(x, y) {
                            if board.tiles[ny][nx].owner == b {
                                shares_border = true;
                                break 'outer;
                            }
                        }
                    }
                }
            }

            if !shares_border {
                // Refund troops to both players
                if a != NO_OWNER {
                    if let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == a) {
                        let refund = if net_troops > 0 {
                            net_troops as u32
                        } else {
                            0
                        };
                        player.troops += refund;
                        if refund > 0 {
                            bevy::log::debug!("Refunded {} troops to Player {}", refund, a);
                        }
                    }
                }

                if b != NO_OWNER {
                    if let Some((_, mut player)) = players.iter_mut().find(|(_, p)| p.id == b) {
                        let refund = if net_troops < 0 {
                            (-net_troops) as u32
                        } else {
                            0
                        };
                        player.troops += refund;
                        if refund > 0 {
                            bevy::log::debug!("Refunded {} troops to Player {}", refund, b);
                        }
                    }
                }

                expansions.clear_border(a, b);
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
        .add_systems(Startup, (setup, initial_border_calculation).chain())
        .add_systems(Update, (update_game, update_tiles, update_player_info))
        .run();
}
