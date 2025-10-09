// Copy board data to render texture shader - GPU-to-GPU copy for rendering

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board: array<u32>;
@group(0) @binding(2) var render_texture: texture_storage_2d<r32uint, write>;

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pos = global_id.xy;

    // Bounds check
    if pos.x >= params.board_width || pos.y >= params.board_height {
        return;
    }

    let index = pos.y * params.board_width + pos.x;
    let tile_data = board[index];

    // Write the raw u32 tile data directly to the texture
    textureStore(render_texture, vec2<i32>(pos), vec4<u32>(tile_data, 0, 0, 0));
}
