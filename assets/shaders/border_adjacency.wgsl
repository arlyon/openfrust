// Border adjacency shader - builds a bit-packed adjacency matrix

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board: array<u32>;
@group(0) @binding(2) var<storage, read_write> adjacency: array<atomic<u32>>;

// Unpack a 16-bit tile from a u32 containing two tiles
fn unpack_tile_data(linear_idx: u32) -> u32 {
    let packed_idx = linear_idx / 2u;
    let sub_idx = linear_idx % 2u;
    let packed_val = board[packed_idx];
    return (packed_val >> (sub_idx * 16u)) & 0xFFFFu;
}

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

// Calculates the word index and bit index within that word for a pair of players
// This matches ActiveExpansions::pair_index logic: N*x - (x*(x+1))/2 + y - x - 1
fn get_packed_indices(p1: u32, p2: u32) -> vec2<u32> {
    if (p1 == p2) { return vec2(0xFFFFFFFFu, 0u); } // Invalid index for self-comparison

    let x = min(p1, p2);
    let y = max(p1, p2);

    // Formula for a packed triangular matrix (without diagonal)
    let n = params.num_entities;
    let linear_bit_index = n * x - (x * (x + 1u)) / 2u + y - x - 1u;

    let word_index = linear_bit_index / 32u;
    let bit_in_word = linear_bit_index % 32u;

    return vec2(word_index, bit_in_word);
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    let index = get_index(pos.x, pos.y);
    let tile_data = unpack_tile_data(index);
    let owner = get_owner(tile_data);

    // Check 2 neighbors (right and down) to process each border only once
    let offsets = array<vec2<i32>, 2>(
        vec2<i32>(1, 0),   // East
        vec2<i32>(0, 1)    // South
    );

    for (var i = 0; i < 2; i++) {
        let npos = vec2<i32>(pos) + offsets[i];
        if (in_bounds(npos.x, npos.y)) {
            let n_index = get_index(u32(npos.x), u32(npos.y));
            let n_tile_data = unpack_tile_data(n_index);
            let n_owner = get_owner(n_tile_data);

            if (owner != n_owner) {
                // Calculate where to set the bit for this pair
                let indices = get_packed_indices(owner, n_owner);
                if (indices.x == 0xFFFFFFFFu) { continue; } // Skip invalid indices

                let word_index = indices.x;
                let bit_mask = 1u << indices.y;

                // Atomically OR the bit into the correct u32 word
                // This is thread-safe for multiple borders contributing to the same word
                atomicOr(&adjacency[word_index], bit_mask);
            }
        }
    }
}
