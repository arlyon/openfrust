#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var map_texture: texture_2d<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> border_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> border_thickness: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> texture_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage> player_colors: array<vec4<f32>>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;

    // Convert UV to pixel coordinates
    let pixel_coord = vec2<i32>(uv * texture_size);

    // Load the center tile data (u16 stored in R16Uint)
    let center_tile_data = textureLoad(map_texture, pixel_coord, 0).r;

    // Extract owner ID from bits 0-11 (mask: 0x0FFF)
    let center_owner = center_tile_data & 0x0FFFu;

    // Look up the color for this owner
    let center_color = player_colors[center_owner];

    // Load neighboring pixels for border detection (4-directional)
    let offset = i32(border_thickness);

    let top_owner = textureLoad(map_texture, pixel_coord + vec2<i32>(0, offset), 0).r & 0x0FFFu;
    let bottom_owner = textureLoad(map_texture, pixel_coord - vec2<i32>(0, offset), 0).r & 0x0FFFu;
    let left_owner = textureLoad(map_texture, pixel_coord - vec2<i32>(offset, 0), 0).r & 0x0FFFu;
    let right_owner = textureLoad(map_texture, pixel_coord + vec2<i32>(offset, 0), 0).r & 0x0FFFu;

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
