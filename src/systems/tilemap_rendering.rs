use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

use crate::types::*;
use crate::{BOARD_HEIGHT, BOARD_WIDTH, TILE_SIZE};

/// Creates the tilemap for rendering the game board
pub fn setup_tilemap(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    board: Res<Board>,
    player_colors: Res<PlayerColorMap>,
) {
    let texture_handle: Handle<Image> = asset_server.load("tile.png");

    let map_size = TilemapSize {
        x: BOARD_WIDTH as u32,
        y: BOARD_HEIGHT as u32,
    };

    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(map_size);

    // Spawn all tiles
    for y in 0..BOARD_HEIGHT {
        for x in 0..BOARD_WIDTH {
            let tile_pos = TilePos {
                x: x as u32,
                y: y as u32,
            };

            let owner = board.get(x, y).owner() as usize;
            let color = player_colors.0[owner];

            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    texture_index: TileTextureIndex(0),
                    color: TileColor(color),
                    ..Default::default()
                })
                .id();

            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    let tile_size = TilemapTileSize {
        x: TILE_SIZE,
        y: TILE_SIZE,
    };
    let grid_size = tile_size.into();
    let map_type = TilemapType::Square;

    // Use chunking for better performance with large maps
    let chunk_size = if BOARD_WIDTH >= 128 || BOARD_HEIGHT >= 128 {
        UVec2::new(64, 64)
    } else {
        UVec2::new(32, 32)
    };

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(texture_handle),
        tile_size,
        anchor: TilemapAnchor::Center,
        render_settings: TilemapRenderSettings {
            render_chunk_size: chunk_size,
            ..Default::default()
        },
        ..Default::default()
    });
}

/// Updates tile colors when ownership changes
pub fn update_tilemap_tiles(
    mut tile_change_reader: MessageReader<TileChangeMessage>,
    tile_storage_query: Query<&TileStorage>,
    mut tile_query: Query<&mut TileColor>,
    color_map: Res<PlayerColorMap>,
) {
    let Ok(tile_storage) = tile_storage_query.single() else {
        return;
    };

    for message in tile_change_reader.read() {
        let tile_pos = TilePos {
            x: message.x as u32,
            y: message.y as u32,
        };

        if let Some(tile_entity) = tile_storage.get(&tile_pos) {
            if let Ok(mut tile_color) = tile_query.get_mut(tile_entity) {
                tile_color.0 = color_map.0[message.new_owner];
            }
        }
    }
}
