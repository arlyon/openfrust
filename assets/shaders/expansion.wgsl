// Expansion compute shader - processes territory conquest in parallel

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> front_lookup: array<i32>; // Packed: NUM_PAIRS with sign
@group(0) @binding(2) var<storage, read_write> conquest_counters: array<atomic<u32>>; // Packed: NUM_PAIRS
@group(0) @binding(3) var<storage, read> board_in: array<u32>;
@group(0) @binding(4) var<storage, read_write> board_out: array<u32>;
@group(0) @binding(5) var<storage, read> map_terrain: array<u32>;

// Unpack a 16-bit tile from a u32 containing two tiles
fn unpack_tile_data(linear_idx: u32) -> u32 {
    let packed_idx = linear_idx / 2u;
    let sub_idx = linear_idx % 2u;
    let packed_val = board_in[packed_idx];
    return (packed_val >> (sub_idx * 16u)) & 0xFFFFu;
}

// Pack two 16-bit tiles into a u32
fn pack_tiles(tile1: u32, tile2: u32) -> u32 {
    return ((tile2 & 0xFFFFu) << 16u) | (tile1 & 0xFFFFu);
}

// Extract owner ID from tile data (bits 0-11)
fn get_owner(tile_data: u32) -> u32 {
    return tile_data & 0x0FFFu;
}

// Set owner ID in tile data
fn set_owner(tile_data: u32, owner_id: u32) -> u32 {
    return (tile_data & 0xF000u) | (owner_id & 0x0FFFu);
}

// Extract terrain difficulty from tile data (bits 12-14)
fn get_terrain_diff(tile_data: u32) -> u32 {
    return (tile_data >> 12u) & 0x7u;
}

// Convert terrain type (0-7) to difficulty multiplier (0.5-2.0)
fn terrain_to_difficulty(terrain_type: u32) -> f32 {
    return 0.5 + (f32(terrain_type) / 7.0) * 1.5;
}

// Check if a position is within bounds
fn in_bounds(x: i32, y: i32) -> bool {
    return x >= 0 && x < i32(params.board_width) && y >= 0 && y < i32(params.board_height);
}

// Get index for a 2D position
fn get_index(x: u32, y: u32) -> u32 {
    return y * params.board_width + x;
}

// Get packed pair index for two players (triangular packing)
// Returns (index, needs_negation) where needs_negation indicates if we should flip the sign
fn get_pair_index(p1: u32, p2: u32) -> vec2<u32> {
    if (p1 == p2) { return vec2(0xFFFFFFFFu, 0u); } // Invalid

    let x = min(p1, p2);
    let y = max(p1, p2);
    let n = params.num_entities;

    // Same formula as ActiveExpansions::pair_index
    let index = n * x - (x * (x + 1u)) / 2u + y - x - 1u;

    // If p1 > p2, we need to negate the value since the stored value represents x->y
    let needs_flip = u32(p1 > p2);

    return vec2(index, needs_flip);
}

// --- MapTile Decoding ---
// MapTile bitfield layout (u8):
// Bit 7: is_land
// Bit 6: is_shoreline
// Bit 5: is_ocean
// Bits 0-4: magnitude (0-31)

fn get_map_tile_at(coord: vec2<i32>) -> u32 {
    // Bounds check
    if (coord.x < 0 || coord.x >= i32(params.board_width) || coord.y < 0 || coord.y >= i32(params.board_height)) {
        return 0u; // Return a default (water, no magnitude) tile if out of bounds
    }

    // Calculate 1D index from 2D coordinates
    let linear_index = u32(coord.y) * params.board_width + u32(coord.x);

    // Unpack terrain data (4 u8 MapTiles per u32)
    let packed_idx = linear_index / 4u;
    let sub_idx = linear_index % 4u;
    let packed_val = map_terrain[packed_idx];
    let tile_byte = (packed_val >> (sub_idx * 8u)) & 0xFFu;

    return tile_byte;
}

fn is_land(tile: u32) -> bool {
    return (tile & 0x80u) != 0u; // Bit 7
}

fn is_ocean(tile: u32) -> bool {
    return (tile & 0x20u) != 0u; // Bit 5
}

fn get_magnitude(tile: u32) -> u32 {
    return tile & 0x1Fu; // Bits 0-4
}
// --- End MapTile Decoding ---

// Process a single tile and return updated tile data
fn process_tile(x: u32, y: u32, tile_data: u32) -> u32 {
    // Get this tile's physical terrain data from the map
    let map_tile = get_map_tile_at(vec2<i32>(i32(x), i32(y)));

    // Oceans are impassable and cannot be conquered
    if is_ocean(map_tile) {
        return tile_data;
    }

    let current_owner = get_owner(tile_data);
    var new_tile_data = tile_data;

    // Check all 8 neighbors to see if this tile can be conquered
    let offsets = array<vec2<i32>, 8>(
        vec2<i32>(-1, -1), vec2<i32>(0, -1), vec2<i32>(1, -1),
        vec2<i32>(-1,  0),                   vec2<i32>(1,  0),
        vec2<i32>(-1,  1), vec2<i32>(0,  1), vec2<i32>(1,  1)
    );

    // Find all neighboring tiles with different owners (potential attackers)
    for (var i = 0; i < 8; i++) {
        let nx = i32(x) + offsets[i].x;
        let ny = i32(y) + offsets[i].y;

        if in_bounds(nx, ny) {
            let neighbor_index = get_index(u32(nx), u32(ny));
            let neighbor_tile = unpack_tile_data(neighbor_index);
            let neighbor_owner = get_owner(neighbor_tile);

            if neighbor_owner != current_owner {
                // Get the packed index for this pair
                let pair_info = get_pair_index(neighbor_owner, current_owner);
                if (pair_info.x == 0xFFFFFFFFu) { continue; }

                let pair_idx = pair_info.x;
                let needs_flip = pair_info.y;

                // Read the signed value and flip if needed
                var net_troops = front_lookup[pair_idx];
                if (needs_flip == 1u) {
                    net_troops = -net_troops;
                }

                // Positive means neighbor is attacking current_owner
                if net_troops > 0 {
                    let tiles_to_conquer = u32(net_troops);

                    // This neighbor is actively attacking - try to claim a conquest slot
                    let old_counter = atomicAdd(&conquest_counters[pair_idx], 1u);

                    // If we successfully claimed a slot within the allocation
                    if old_counter < tiles_to_conquer {
                        // Conquer this tile
                        new_tile_data = set_owner(tile_data, neighbor_owner);
                        break; // Only one attacker can conquer per tick
                    }
                }
            }
        }
    }

    return new_tile_data;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Each thread now processes 2 tiles (one packed u32) to avoid race conditions
    let packed_x = global_id.x * 2u;
    let y = global_id.y;

    // Bounds check for the packed tile pair
    if packed_x >= params.board_width || y >= params.board_height {
        return;
    }

    // Calculate the packed index
    let index1 = get_index(packed_x, y);
    let packed_idx = index1 / 2u;

    // Read the packed value containing two tiles
    let packed_val = board_in[packed_idx];
    let tile1_data = packed_val & 0xFFFFu;
    let tile2_data = (packed_val >> 16u) & 0xFFFFu;

    // Process first tile
    let new_tile1 = process_tile(packed_x, y, tile1_data);

    // Process second tile if it's within bounds
    var new_tile2 = tile2_data;
    if packed_x + 1u < params.board_width {
        new_tile2 = process_tile(packed_x + 1u, y, tile2_data);
    }

    // Pack and write the result
    let new_packed = (new_tile2 << 16u) | new_tile1;
    board_out[packed_idx] = new_packed;
}
