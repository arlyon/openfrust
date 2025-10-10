// Copy shader to sync board_out to board_render for rendering

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board_out: array<u32>;
@group(0) @binding(2) var<storage, read_write> board_render: array<u32>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Data is packed: 2 tiles per u32, so the array width is board_width/2
    let packed_width = params.board_width / 2u;
    let index = global_id.y * packed_width + global_id.x;
    if (index < arrayLength(&board_out)) {
        board_render[index] = board_out[index];
    }
}
