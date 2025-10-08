#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var map_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var map_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> border_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> border_thickness: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<uniform> texture_size: vec2<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let texel_size = 1.0 / texture_size;

    // Sample the center pixel
    let center_color = textureSample(map_texture, map_sampler, uv);

    // Sample neighboring pixels for border detection (4-directional)
    let offset = texel_size * border_thickness;

    let top = textureSample(map_texture, map_sampler, uv + vec2<f32>(0.0, offset.y));
    let bottom = textureSample(map_texture, map_sampler, uv - vec2<f32>(0.0, offset.y));
    let left = textureSample(map_texture, map_sampler, uv - vec2<f32>(offset.x, 0.0));
    let right = textureSample(map_texture, map_sampler, uv + vec2<f32>(offset.x, 0.0));

    // Check if any neighbor is significantly different
    let color_threshold = 0.01;
    var is_border = false;

    if (distance(center_color.rgb, top.rgb) > color_threshold ||
        distance(center_color.rgb, bottom.rgb) > color_threshold ||
        distance(center_color.rgb, left.rgb) > color_threshold ||
        distance(center_color.rgb, right.rgb) > color_threshold) {
        is_border = true;
    }

    // If we're at a border, darken the color
    if (is_border) {
        return vec4<f32>(center_color.rgb * border_color.rgb, center_color.a);
    } else {
        return center_color;
    }
}
