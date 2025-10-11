// Process results shader - calculates player stats with fully parallel workgroup reduction

struct SimParams {
    board_width: u32,
    board_height: u32,
    expansion_rate: f32,
    num_entities: u32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var<storage, read> board_out: array<u32>;
@group(0) @binding(2) var<storage, read_write> player_tile_counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> player_sum_x_low: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> player_sum_x_high: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> player_sum_y_low: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> player_sum_y_high: array<atomic<u32>>;

// Workgroup shared memory - all threads collaborate to aggregate here first
// Array sizes must match NUM_ENTITIES (101)
var<workgroup> local_counts: array<atomic<u32>, 101>;
var<workgroup> local_sum_x: array<atomic<u32>, 101>;
var<workgroup> local_sum_y: array<atomic<u32>, 101>;

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
fn main(@builtin(global_invocation_id) global_id: vec3<u32>, @builtin(local_invocation_index) local_idx: u32) {
    // --- STEP 1: Initialize workgroup memory in parallel ---
    // Each of the first `num_entities` threads zeros out one entry
    if (local_idx < params.num_entities) {
        atomicStore(&local_counts[local_idx], 0u);
        atomicStore(&local_sum_x[local_idx], 0u);
        atomicStore(&local_sum_y[local_idx], 0u);
    }
    workgroupBarrier(); // Ensure all memory is zero before proceeding

    // --- STEP 2: All threads read global memory and aggregate into workgroup memory ---
    let pos = global_id.xy;
    if (pos.x < params.board_width && pos.y < params.board_height) {
        let index = pos.y * params.board_width + pos.x;
        let tile_out = unpack_tile_data(index);
        let owner_out = get_owner(tile_out);

        // Perform fast atomic adds to shared workgroup memory
        atomicAdd(&local_counts[owner_out], 1u);
        atomicAdd(&local_sum_x[owner_out], pos.x);
        atomicAdd(&local_sum_y[owner_out], pos.y);
    }
    workgroupBarrier(); // Ensure all threads have finished their aggregation

    // --- STEP 3: Write final results from workgroup to global memory in parallel ---
    // Each of the first `num_entities` threads is responsible for writing out one player's stats
    if (local_idx < params.num_entities) {
        let count = atomicLoad(&local_counts[local_idx]);
        if (count > 0u) {
            // Add this workgroup's count to the global total for this player
            atomicAdd(&player_tile_counts[local_idx], count);

            // Add this workgroup's sum_x to the global total, handling the 64-bit carry
            let sum_x = atomicLoad(&local_sum_x[local_idx]);
            let old_low_x = atomicAdd(&player_sum_x_low[local_idx], sum_x);
            if (old_low_x > (0xFFFFFFFFu - sum_x)) {
                atomicAdd(&player_sum_x_high[local_idx], 1u);
            }

            // Add this workgroup's sum_y to the global total, handling the 64-bit carry
            let sum_y = atomicLoad(&local_sum_y[local_idx]);
            let old_low_y = atomicAdd(&player_sum_y_low[local_idx], sum_y);
            if (old_low_y > (0xFFFFFFFFu - sum_y)) {
                atomicAdd(&player_sum_y_high[local_idx], 1u);
            }
        }
    }
}
