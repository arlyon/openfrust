// Expansion compute shader - processes territory conquest in parallel

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    _padding: u32,
}

struct Front {
    attacker_id: u32,
    defender_id: u32,
    tiles_to_conquer: u32,
    _padding: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> fronts: array<Front>;
@group(0) @binding(2) var<storage, read_write> conquer_counters: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read> board_in: array<u32>;
@group(0) @binding(4) var<storage, read_write> board_out: array<u32>;

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

// Find the front index for a given attacker-defender pair
// Returns NUM_PAIRS if not found (invalid index)
fn find_front_index(attacker: u32, defender: u32) -> u32 {
    for (var i = 0u; i < arrayLength(&fronts); i++) {
        if fronts[i].attacker_id == attacker && fronts[i].defender_id == defender {
            return i;
        }
    }
    return 999999u; // Invalid index
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    // Calculate 1D index
    let index = get_index(pos.x, pos.y);

    // Read the current tile data
    let tile_data = board_in[index];
    let current_owner = get_owner(tile_data);

    // By default, copy the current state
    var new_tile_data = tile_data;

    // Check all 8 neighbors to see if this tile can be conquered
    let offsets = array<vec2<i32>, 8>(
        vec2<i32>(-1, -1), vec2<i32>(0, -1), vec2<i32>(1, -1),
        vec2<i32>(-1,  0),                   vec2<i32>(1,  0),
        vec2<i32>(-1,  1), vec2<i32>(0,  1), vec2<i32>(1,  1)
    );

    var potential_attackers: array<u32, 8>;
    var attacker_count = 0u;

    // Find all neighboring tiles with different owners (potential attackers)
    for (var i = 0; i < 8; i++) {
        let nx = i32(pos.x) + offsets[i].x;
        let ny = i32(pos.y) + offsets[i].y;

        if in_bounds(nx, ny) {
            let neighbor_index = get_index(u32(nx), u32(ny));
            let neighbor_owner = get_owner(board_in[neighbor_index]);

            if neighbor_owner != current_owner {
                // Check if there's an active front for this pair
                let front_idx = find_front_index(neighbor_owner, current_owner);

                if front_idx < arrayLength(&fronts) && fronts[front_idx].tiles_to_conquer > 0u {
                    // This neighbor is actively attacking - try to claim a conquest slot
                    let old_counter = atomicAdd(&conquer_counters[front_idx], 1u);

                    // If we successfully claimed a slot within the allocation
                    if old_counter < fronts[front_idx].tiles_to_conquer {
                        // Conquer this tile
                        new_tile_data = set_owner(tile_data, neighbor_owner);
                        break; // Only one attacker can conquer per tick
                    }
                }
            }
        }
    }

    // Write the result
    board_out[index] = new_tile_data;
}
