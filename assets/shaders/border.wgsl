#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage, read> board_data: array<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> border_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> border_thickness: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> texture_size: vec2<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage> player_colors: array<vec4<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage, read> map_terrain: array<u32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<uniform> time: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<uniform> enable_water_animation: u32;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var<uniform> enable_players: u32;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var<uniform> enable_sphere_projection: u32;


// --- Simplex Noise Functions ---
fn mod289(x: vec2<f32>) -> vec2<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}
fn mod289_3(x: vec3<f32>) -> vec3<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}
fn permute3(x: vec3<f32>) -> vec3<f32> {
    return mod289_3(((x * 34.0) + 1.0) * x);
}
fn simplexNoise2(v: vec2<f32>) -> f32 {
    let C = vec4<f32>(0.2113248654, 0.3660254037, -0.5773502691, 0.0243902439);
    var i = floor(v + dot(v, C.yy));
    let x0 = v - i + dot(i, C.xx);
    let i1 = select(vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), x0.x > x0.y);
    let x1 = x0 - i1 + C.xx;
    let x2 = x0 - 1.0 + C.yy;
    i = mod289(i);
    var p = permute3(permute3(i.y + vec3<f32>(0.0, i1.y, 1.0)) + i.x + vec3<f32>(0.0, i1.x, 1.0));
    var m = max(0.5 - vec3<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2)), vec3<f32>(0.0));
    m = m * m;
    m = m * m;
    let x = 2.0 * fract(p * C.www) - 1.0;
    let h = abs(x) - 0.5;
    let ox = floor(x + 0.5);
    let a0 = x - ox;
    m = m * (1.792842914 - 0.853734720 * (a0 * a0 + h * h));
    let g = vec3<f32>(a0.x * x0.x + h.x * x0.y, a0.y * x1.x + h.y * x1.y, a0.z * x2.x + h.z * x2.y);
    return 130.0 * dot(m, g);
}

// UPDATED: Now takes a vec2<f32> coordinate instead of a UV.
fn animated_simplex_noise(coord: vec2<f32>, speed: f32, scale: f32) -> f32 {
    let p = coord * scale; // Use discrete pixel coords
    let motion = vec2<f32>(time * speed, -time * speed * 0.7);
    let noise = simplexNoise2(p + motion);
    return (noise + 1.0) * 0.5;
}
// --- End Simplex Noise Functions ---

fn get_map_tile_at(coord: vec2<i32>) -> u32 {
    if (coord.x < 0 || coord.x >= i32(texture_size.x) || coord.y < 0 || coord.y >= i32(texture_size.y)) {
        return 0u;
    }
    let linear_index = u32(coord.y) * u32(texture_size.x) + u32(coord.x);
    let packed_idx = linear_index / 4u;
    let sub_idx = linear_index % 4u;
    let packed_val = map_terrain[packed_idx];
    return (packed_val >> (sub_idx * 8u)) & 0xFFu;
}

fn is_land(tile: u32) -> bool { return (tile & 0x80u) != 0u; }
fn is_ocean(tile: u32) -> bool { return (tile & 0x20u) != 0u; }
fn get_magnitude(tile: u32) -> u32 { return tile & 0x1Fu; }

// UPDATED: Signature changed to accept discrete integer coordinates
fn get_terrain_color(tile: u32, coord: vec2<i32>) -> vec4<f32> {
    if (is_land(tile)) {
        let mag = get_magnitude(tile);
        if (mag < 10u) {
            let brightness = 1.0 + (f32(mag) / 10.0) * 0.1;
            return vec4<f32>(0.6, 0.8, 0.4, 1.0) * brightness;
        } else if (mag < 20u) {
            let brightness = 1.0 + (f32(mag - 10u) / 10.0) * 0.15;
            return vec4<f32>(0.7, 0.6, 0.4, 1.0) * brightness;
        } else {
            let brightness = 1.0 + (f32(mag - 20u) / 11.0) * 0.5;
            return vec4<f32>(0.5, 0.5, 0.5, 1.0) * brightness;
        }
    } else if (is_ocean(tile)) {
        let base_color = vec3<f32>(0.01, 0.08, 0.23);

        // Check if water animation is enabled
        if (enable_water_animation != 0u) {
            let highlight_color = vec3<f32>(0.1, 0.2, 0.7);
            let f_coord = vec2<f32>(coord); // Cast integer coord to float once

            // UPDATED: Use the discrete f_coord and much smaller scale values.
            // These values create blocky wave patterns roughly 25-50 pixels in size.
            let wave1 = animated_simplex_noise(f_coord, 0.4, 0.5); // noise
            let wave2 = animated_simplex_noise(f_coord, 0.2, 0.04); // fine waves
            let wave3 = animated_simplex_noise(f_coord, 0.1, 0.001); // broad

            let combined_waves = (
                wave1 * 1.0
                + wave2 * 0.2
                + wave3 * 0.4
            ) * 0.5;
            var final_color = mix(base_color, highlight_color, combined_waves * 0.5);

            // UPDATED: Specular highlight is also calculated on the discrete grid.
            let specular_scale = 0.09; // Makes glints about 6-7 pixels wide
            let specular_stretch = vec2<f32>(1.0, 2.5); // Stretch horizontally
            let specular_noise = animated_simplex_noise(f_coord * specular_stretch, 0.3, specular_scale);

            let glint_amount = pow(specular_noise, 32.0);
            let specular_color = vec3<f32>(1.0, 0.95, 0.8);
            final_color += glint_amount * specular_color * 1.5;

            return vec4<f32>(final_color, 1.0);
        } else {
            // Simple static water color when animation is disabled
            return vec4<f32>(base_color, 1.0);
        }
    } else {
        // Lake - lighter blue
        return vec4<f32>(0.3, 0.5, 0.7, 1.0);
    }
}

fn get_owner_at(coord: vec2<i32>) -> u32 {
    if (coord.x < 0 || coord.x >= i32(texture_size.x) || coord.y < 0 || coord.y >= i32(texture_size.y)) {
        return 0u;
    }
    let linear_index = u32(coord.y) * u32(texture_size.x) + u32(coord.x);
    let packed_idx = linear_index / 2u;
    let sub_idx = linear_index % 2u;
    let packed_val = board_data[packed_idx];
    let tile_data = (packed_val >> (sub_idx * 16u)) & 0xFFFFu;
    return tile_data & 0x0FFFu;
}

// --- ADD THESE HELPER FUNCTIONS FOR SPHERE PROJECTION ---
const PI: f32 = 3.1415926535;

// Solves ray-sphere intersection. Returns distance `t`, or a negative value on miss.
fn intersect_sphere(ro: vec3<f32>, rd: vec3<f32>, radius: f32) -> f32 {
    let b = dot(ro, rd);
    let c = dot(ro, ro) - radius * radius;
    let disc = b * b - c; // simplified discriminant
    if (disc < 0.0) {
        return -1.0;
    }
    return -b - sqrt(disc); // return smallest positive root
}

// --- UPDATE THE FRAGMENT SHADER ---
@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    var uv = mesh.uv;

    if (enable_sphere_projection != 0u) {
        // Remap screen UV from [0,1] to [-1,1] to represent a view plane
        var screen_coords = (mesh.uv - 0.5) * 2.0;

        // --- ADD THESE TWO LINES ---
        let aspect_ratio = texture_size.x / texture_size.y;
        screen_coords.x *= aspect_ratio;

        // Setup camera ray
        // Camera at (0,0,-2.5) looking towards origin. View plane is at z = -1.0
        let ro = vec3<f32>(0.0, 0.0, -2.5);
        let rd = normalize(vec3<f32>(screen_coords, 1.5)); // z component controls FOV

        let t = intersect_sphere(ro, rd, 1.0); // Intersect with a unit sphere

        if (t < 0.0) {
            // Ray missed the sphere. Return transparent to see through it.
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }

        // Calculate intersection point on sphere
        var pos = ro + t * rd;

        // Add a simple rotation around the Y axis based on time
        let time_scaled = time * 0.1;
        let cos_t = cos(time_scaled);
        let sin_t = sin(time_scaled);
        let rot_y = mat3x3<f32>(
            cos_t, 0.0, sin_t,
            0.0,   1.0, 0.0,
            -sin_t, 0.0, cos_t
        );
        pos = rot_y * pos;

        // Convert 3D sphere point to 2D UV coordinates (Equirectangular projection)
        let lat = asin(pos.y);         // Latitude from y-coordinate, range: [-PI/2, PI/2]
        let lon = atan2(pos.x, pos.z); // Longitude from x and z, range: [-PI, PI]

        // Map latitude and longitude back to UV range [0,1]
        uv.x =  0.5 - lon / (2.0 * PI);
        uv.y = lat / PI + 0.5; // Invert Y to match typical texture coordinates
    }

    // This is the key: the discrete integer coordinate for the current world pixel.
    let pixel_coord = vec2<i32>(uv * texture_size);

    let center_owner = get_owner_at(pixel_coord);
    let center_terrain = get_map_tile_at(pixel_coord);

    // UPDATED: Pass the discrete pixel_coord instead of the continuous uv.
    var terrain_color = get_terrain_color(center_terrain, pixel_coord);

    // Shoreline Effect
    let center_is_land = is_land(center_terrain);
    if (center_is_land) {
        let top_terrain = get_map_tile_at(pixel_coord + vec2<i32>(0, 1));
        let bottom_terrain = get_map_tile_at(pixel_coord - vec2<i32>(0, 1));
        let left_terrain = get_map_tile_at(pixel_coord - vec2<i32>(1, 0));
        let right_terrain = get_map_tile_at(pixel_coord + vec2<i32>(1, 0));

        let is_shoreline = !is_land(top_terrain) || !is_land(bottom_terrain) || !is_land(left_terrain) || !is_land(right_terrain);

        if (is_shoreline) {
            let sand_color = vec4<f32>(0.85, 0.8, 0.6, 1.0);
            terrain_color = mix(terrain_color, sand_color, 0.7);
        }
    }

    var base_color: vec4<f32>;
    if (enable_players != 0u) {
        let player_color = player_colors[center_owner];
        if (center_owner == 0u) {
            base_color = terrain_color;
        } else {
            base_color = mix(terrain_color, player_color, 0.6);
        }
    } else {
        // When player rendering is disabled, just show terrain
        base_color = terrain_color;
    }

    // Only render borders if player rendering is enabled
    if (enable_players != 0u) {
        let offset = i32(border_thickness);
        let top_owner = get_owner_at(pixel_coord + vec2<i32>(0, offset));
        let bottom_owner = get_owner_at(pixel_coord - vec2<i32>(0, offset));
        let left_owner = get_owner_at(pixel_coord - vec2<i32>(offset, 0));
        let right_owner = get_owner_at(pixel_coord + vec2<i32>(offset, 0));

        var is_owner_border = (center_owner != top_owner || center_owner != bottom_owner || center_owner != left_owner || center_owner != right_owner);

        if (is_owner_border) {
            return vec4<f32>(base_color.rgb * border_color.rgb, base_color.a);
        }
    }

    return base_color;
}
