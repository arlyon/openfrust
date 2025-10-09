// Seed shader - initializes board from sparse player starting positions

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
    seed_count: u32,
    _padding1: u32,
    _padding2: u32,
    _padding3: u32,
}

struct SeedTile {
    pos: vec2<u32>,
    data: u32,
    _padding: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> seeds: array<SeedTile>;
@group(0) @binding(2) var<storage, read_write> board_in: array<u32>;

const NO_OWNER: u32 = 0u;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    let index = pos.y * params.board_width + pos.x;

    // Default to wilderness (owner 0)
    var tile_data = NO_OWNER;

    // Check if this position matches any seed
    // This is O(n) but only runs once at startup with ~100 seeds
    for (var i = 0u; i < params.seed_count; i++) {
        if seeds[i].pos.x == pos.x && seeds[i].pos.y == pos.y {
            tile_data = seeds[i].data;
            break;
        }
    }

    board_in[index] = tile_data;
}
