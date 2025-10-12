#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage, read> board_data: array<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> border_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> border_thickness: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> texture_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage> player_colors: array<vec4<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage, read> map_terrain: array<u32>;

// MapTile bitfield layout (u8):
// Bit 7: is_land
// Bit 6: is_shoreline
// Bit 5: is_ocean
// Bits 0-4: magnitude (0-31)

fn get_map_tile_at(coord: vec2<i32>) -> u32 {
    // Bounds check
    if (coord.x < 0 || coord.x >= i32(texture_size.x) || coord.y < 0 || coord.y >= i32(texture_size.y)) {
        return 0u;
    }

    // Calculate 1D index from 2D coordinates
    let linear_index = u32(coord.y) * u32(texture_size.x) + u32(coord.x);

    // Unpack terrain data (4 u8 MapTiles per u32)
    let packed_idx = linear_index / 4u;
    let sub_idx = linear_index % 4u;
    let packed_val = map_terrain[packed_idx];
    let tile_byte = (packed_val >> (sub_idx * 8u)) & 0xFFu;

    return tile_byte;
}

fn is_land(tile: u32) -> bool {
    return (tile & 0x80u) != 0u; // Bit 7
}

fn is_ocean(tile: u32) -> bool {
    return (tile & 0x20u) != 0u; // Bit 5
}

fn get_magnitude(tile: u32) -> u32 {
    return tile & 0x1Fu; // Bits 0-4
}

fn get_terrain_color(tile: u32) -> vec4<f32> {
    if (is_land(tile)) {
        let mag = get_magnitude(tile);
        if (mag < 10u) {
            // Plains - light green
            return vec4<f32>(0.6, 0.8, 0.4, 1.0);
        } else if (mag < 20u) {
            // Highland - tan/brown
            return vec4<f32>(0.7, 0.6, 0.4, 1.0);
        } else {
            // Mountain - gray
            return vec4<f32>(0.5, 0.5, 0.5, 1.0);
        }
    } else if (is_ocean(tile)) {
        // Ocean - dark blue
        return vec4<f32>(0.2, 0.3, 0.6, 1.0);
    } else {
        // Lake - lighter blue
        return vec4<f32>(0.3, 0.5, 0.7, 1.0);
    }
}

fn get_owner_at(coord: vec2<i32>) -> u32 {
    // Bounds check to prevent reading out of bounds
    if (coord.x < 0 || coord.x >= i32(texture_size.x) || coord.y < 0 || coord.y >= i32(texture_size.y)) {
        return 0u; // Return wilderness owner if out of bounds
    }

    // Calculate 1D index from 2D pixel coordinates
    let linear_index = u32(coord.y) * u32(texture_size.x) + u32(coord.x);

    // Unpack tile data from packed storage (2 tiles per u32)
    let packed_idx = linear_index / 2u;
    let sub_idx = linear_index % 2u;
    let packed_val = board_data[packed_idx];
    let tile_data = (packed_val >> (sub_idx * 16u)) & 0xFFFFu;

    return tile_data & 0x0FFFu;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let pixel_coord = vec2<i32>(uv * texture_size);

    // Get the center tile owner and terrain
    let center_owner = get_owner_at(pixel_coord);
    let center_terrain = get_map_tile_at(pixel_coord);

    // Get base terrain color
    let terrain_color = get_terrain_color(center_terrain);

    // Get player color (for owned territories)
    let player_color = player_colors[center_owner];

    // Blend terrain with player ownership
    // If wilderness (owner 0), show pure terrain color
    // If owned, blend player color with terrain (60% player, 40% terrain)
    var base_color: vec4<f32>;
    if (center_owner == 0u) {
        base_color = terrain_color;
    } else {
        base_color = mix(terrain_color, player_color, 0.6);
    }

    // Load neighboring pixels for border detection (4-directional)
    let offset = i32(border_thickness);

    let top_owner = get_owner_at(pixel_coord + vec2<i32>(0, offset));
    let bottom_owner = get_owner_at(pixel_coord - vec2<i32>(0, offset));
    let left_owner = get_owner_at(pixel_coord - vec2<i32>(offset, 0));
    let right_owner = get_owner_at(pixel_coord + vec2<i32>(offset, 0));

    // Check if any neighbor has a different owner (border detection)
    var is_border = false;

    if (center_owner != top_owner ||
        center_owner != bottom_owner ||
        center_owner != left_owner ||
        center_owner != right_owner) {
        is_border = true;
    }

    // If we're at a border, darken the color
    if (is_border) {
        return vec4<f32>(base_color.rgb * border_color.rgb, base_color.a);
    } else {
        return base_color;
    }
}
