// Uniforms provided by the CPU
@group(0) @binding(0) var<uniform> board_dims: vec2<u32>;
@group(0) @binding(1) var<uniform> target_coord: vec2<u32>;

// Input board data (read-only)
@group(0) @binding(3) var<storage, read> board_data: array<u32>;

// Output buffer for the resulting player ID
// Using an atomic is simple for a single value, but a regular storage buffer would also work.
@group(0) @binding(2) var<storage, read_write> result_id: atomic<u32>;

const NO_OWNER_SENTINEL: u32 = 0xFFFFFFFFu; // A value to signify "not found" or out of bounds

// Unpack the owner ID from a 16-bit tile data value
// (This logic is from your existing shaders)
fn get_owner(tile_data: u32) -> u32 {
    return tile_data & 0x0FFFu; // Bits 0-11
}

@compute @workgroup_size(1, 1, 1)
fn main() {
    // --- 1. Bounds Check ---
    // Make sure the requested coordinate is within the board dimensions
    if (target_coord.x >= board_dims.x || target_coord.y >= board_dims.y) {
        atomicStore(&result_id, NO_OWNER_SENTINEL);
        return;
    }

    // --- 2. Calculate Index ---
    // Convert the 2D coordinate into a 1D array index
    let linear_index = target_coord.y * board_dims.x + target_coord.x;

    // --- 3. Unpack Data ---
    // Figure out which u32 contains our tile and whether it's the high or low 16 bits
    let packed_idx = linear_index / 2u;
    let sub_idx = linear_index % 2u;

    // Check that we don't read past the end of the board_data buffer
    if (packed_idx >= arrayLength(&board_data)) {
        atomicStore(&result_id, NO_OWNER_SENTINEL);
        return;
    }

    let packed_val = board_data[packed_idx];
    let tile_data = (packed_val >> (sub_idx * 16u)) & 0xFFFFu;
    let owner_id = get_owner(tile_data);

    // --- 4. Store Result ---
    // Atomically write the found owner ID to the output buffer
    atomicStore(&result_id, owner_id);
}
