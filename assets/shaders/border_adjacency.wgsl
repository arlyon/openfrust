// Border adjacency shader - builds an adjacency matrix showing which players border each other

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board: array<u32>;
@group(0) @binding(2) var<storage, read_write> adjacency: array<atomic<u32>>;

// Extract owner ID from tile data (bits 0-11)
fn get_owner(tile_data: u32) -> u32 {
    return tile_data & 0x0FFFu;
}

// Check if coordinates are within bounds
fn in_bounds(x: i32, y: i32) -> bool {
    return x >= 0 && y >= 0 && x < i32(params.board_width) && y < i32(params.board_height);
}

// Calculate 1D index from 2D coordinates
fn get_index(x: u32, y: u32) -> u32 {
    return y * params.board_width + x;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    let index = get_index(pos.x, pos.y);
    let owner = get_owner(board[index]);

    // Check 4 neighbors (no need for diagonals)
    let offsets = array<vec2<i32>, 4>(
        vec2<i32>(0, -1),  // North
        vec2<i32>(0, 1),   // South
        vec2<i32>(-1, 0),  // West
        vec2<i32>(1, 0)    // East
    );

    for (var i = 0; i < 4; i++) {
        let npos = vec2<i32>(pos) + offsets[i];
        if (in_bounds(npos.x, npos.y)) {
            let n_index = get_index(u32(npos.x), u32(npos.y));
            let n_owner = get_owner(board[n_index]);

            if (owner != n_owner) {
                // This is a border. Flag that these two players are adjacent.
                // Use bidirectional indexing so either player can query the relationship
                let idx1 = owner * params.num_entities + n_owner;
                let idx2 = n_owner * params.num_entities + owner;
                atomicStore(&adjacency[idx1], 1u);
                atomicStore(&adjacency[idx2], 1u);
            }
        }
    }
}
