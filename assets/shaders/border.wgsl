#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage, read> board_data: array<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> border_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> border_thickness: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> texture_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage> player_colors: array<vec4<f32>>;

fn get_owner_at(coord: vec2<i32>) -> u32 {
    // Bounds check to prevent reading out of bounds
    if (coord.x < 0 || coord.x >= i32(texture_size.x) || coord.y < 0 || coord.y >= i32(texture_size.y)) {
        return 0u; // Return wilderness owner if out of bounds
    }

    // Calculate 1D index from 2D pixel coordinates
    let index = u32(coord.y) * u32(texture_size.x) + u32(coord.x);
    let tile_data = board_data[index];
    return tile_data & 0x0FFFu;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let pixel_coord = vec2<i32>(uv * texture_size);

    // Get the center tile owner
    let center_owner = get_owner_at(pixel_coord);

    // Look up the color for this owner
    let center_color = player_colors[center_owner];

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
        return vec4<f32>(center_color.rgb * border_color.rgb, center_color.a);
    } else {
        return center_color;
    }
}
