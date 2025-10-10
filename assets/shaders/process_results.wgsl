// Process results shader - calculates player stats

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

struct GpuPlayerStats {
    tile_count: atomic<u32>,
    sum_x: atomic<u64>,
    sum_y: atomic<u64>,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board_out: array<u32>;
@group(0) @binding(2) var<storage, read_write> player_stats: array<GpuPlayerStats>;

// Unpack a 16-bit tile from a u32 containing two tiles
fn unpack_tile_data(linear_idx: u32) -> u32 {
    let packed_idx = linear_idx / 2u;
    let sub_idx = linear_idx % 2u;
    let packed_val = board_out[packed_idx];
    return (packed_val >> (sub_idx * 16u)) & 0xFFFFu;
}

// Extract owner ID from tile data (bits 0-11)
fn get_owner(tile_data: u32) -> u32 {
    return tile_data & 0x0FFFu;
}

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    // Calculate 1D index
    let index = pos.y * params.board_width + pos.x;

    // Unpack and read tile data after expansion
    let tile_out = unpack_tile_data(index);
    let owner_out = get_owner(tile_out);

    // --- REDUCTION ---
    // Every tile contributes to its owner's statistics
    // This builds a complete picture of the final board state
    atomicAdd(&player_stats[owner_out].tile_count, 1u);
    atomicAdd(&player_stats[owner_out].sum_x, u64(pos.x));
    atomicAdd(&player_stats[owner_out].sum_y, u64(pos.y));
}
