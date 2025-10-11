// Clears atomic/storage buffers to zero at the start of a frame
// This ensures proper synchronization by making buffer clearing part of the GPU pipeline

@group(0) @binding(0) var<storage, read_write> conquest_counters: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> player_stats: array<atomic<u32>>; // Treat GpuPlayerStats as raw u32 for clearing
@group(0) @binding(2) var<storage, read_write> adjacency_matrix: array<atomic<u32>>;

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;

    // Clear conquest counters
    if (idx < arrayLength(&conquest_counters)) {
        atomicStore(&conquest_counters[idx], 0u);
    }

    // Clear adjacency matrix
    if (idx < arrayLength(&adjacency_matrix)) {
        atomicStore(&adjacency_matrix[idx], 0u);
    }

    // Clear player stats. Since GpuPlayerStats is 6 * u32, we need to clear a larger array.
    // The player_stats buffer is bound as a raw atomic<u32> array.
    let stats_length = arrayLength(&player_stats);
    if (idx < stats_length) {
        atomicStore(&player_stats[idx], 0u);
    }
}
