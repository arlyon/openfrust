// Expansion compute shader - Phase 2 stub (will be implemented in Phase 3)

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

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    // Calculate 1D index
    let index = pos.y * params.board_width + pos.x;

    // Stub: just copy input to output for now
    board_out[index] = board_in[index];
}
