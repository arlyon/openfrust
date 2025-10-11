// Clears atomic/storage buffers to zero at the start of a frame
// This ensures proper synchronization by making buffer clearing part of the GPU pipeline

@group(0) @binding(0) var<storage, read_write> conquest_counters: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> player_tile_counts: array<atomic<u32>>;
@group(0) @binding(2) var<storage, read_write> player_sum_x_low: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> player_sum_x_high: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> player_sum_y_low: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> player_sum_y_high: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> adjacency_matrix: array<atomic<u32>>;

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;

    // Clear conquest counters
    if (idx < arrayLength(&conquest_counters)) {
        atomicStore(&conquest_counters[idx], 0u);
    }

    // Clear player statistics buffers (5 separate buffers)
    if (idx < arrayLength(&player_tile_counts)) {
        atomicStore(&player_tile_counts[idx], 0u);
    }
    if (idx < arrayLength(&player_sum_x_low)) {
        atomicStore(&player_sum_x_low[idx], 0u);
    }
    if (idx < arrayLength(&player_sum_x_high)) {
        atomicStore(&player_sum_x_high[idx], 0u);
    }
    if (idx < arrayLength(&player_sum_y_low)) {
        atomicStore(&player_sum_y_low[idx], 0u);
    }
    if (idx < arrayLength(&player_sum_y_high)) {
        atomicStore(&player_sum_y_high[idx], 0u);
    }

    // Clear adjacency matrix
    if (idx < arrayLength(&adjacency_matrix)) {
        atomicStore(&adjacency_matrix[idx], 0u);
    }
}
