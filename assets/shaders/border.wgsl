#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#import "shaders/simplex.wgsl"::animated_simplex_noise

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
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var distance_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var distance_sampler: sampler;

// ============================================================================
// CONSTANTS AND CONFIGURATION
// ============================================================================

const PI: f32 = 3.1415926535;

// --- Terrain Colors ---
// Plains (low elevation, magnitude < 10)
const PLAINS_COLOR: vec3<f32> = vec3<f32>(0.3, 0.4, 0.1);
const PLAINS_BRIGHTNESS_SCALE: f32 = 0.5;

// Highland (medium elevation, magnitude 10-20)
const HIGHLAND_COLOR: vec3<f32> = vec3<f32>(0.65, 0.55, 0.35);
const HIGHLAND_BRIGHTNESS_SCALE: f32 = 0.8;

// Mountain (high elevation, magnitude 20+)
const MOUNTAIN_COLOR: vec3<f32> = vec3<f32>(0.8, 0.8, 0.8);
const MOUNTAIN_BRIGHTNESS_SCALE: f32 = 0.9;

// Shoreline sand color and blend strength
const SAND_COLOR: vec4<f32> = vec4<f32>(0.8, 0.8, 0.6, 1.0);
const SAND_BLEND_AMOUNT: f32 = 0.7;

// --- Ocean and Water Colors ---
// Deep ocean (far from land)
const DEEP_OCEAN_COLOR: vec3<f32> = vec3<f32>(0.01, 0.08, 0.23);

// Coastal water (medium distance from land)
const COASTAL_COLOR: vec3<f32> = vec3<f32>(0.1, 0.5, 0.6);

// Foam/surf (very close to land)
const FOAM_COLOR: vec3<f32> = vec3<f32>(0.9, 0.95, 1.0);

// Rivers and lakes (narrow channels detected by river factor)
// const RIVER_COLOR: vec3<f32> = vec3<f32>(0.05, 0.1, 0.3);
const RIVER_COLOR = COASTAL_COLOR;

// Non-ocean lake color (unused in most cases, legacy support)
const LAKE_COLOR: vec4<f32> = vec4<f32>(DEEP_OCEAN_COLOR, 1.0);

// --- Water Animation Colors ---
// Wave highlight color (mixed in during animation)
const WAVE_HIGHLIGHT_COLOR: vec3<f32> = vec3<f32>(0.1, 0.2, 0.7);
const WAVE_HIGHLIGHT_STRENGTH: f32 = 0.5;

// Specular reflection color (sun glints on water)
const SPECULAR_COLOR: vec3<f32> = vec3<f32>(1.0, 0.95, 0.8);
const SPECULAR_STRENGTH: f32 = 1.5;

// --- Distance Field Parameters ---
// Converts normalized texture values back to pixel distances
const DISTANCE_DENORMALIZE: f32 = 255.0;

// Logarithmic falloff for brightening distant ocean
const FALLOFF_STRENGTH: f32 = 400.0;  // Higher = more brightening at distance
const FALLOFF_OFFSET: f32 = 10.0;     // Prevents log(0) and shifts start point

// --- Coastal Animation Parameters ---
// Foam animation (closest to shore, rapid pulsing)
const FOAM_ANIM_SPEED: f32 = 1.5;           // How fast foam pulses
const FOAM_BASE_DIST: f32 = 1.0;            // Base distance where foam starts (pixels)
const FOAM_ANIM_AMPLITUDE: f32 = 1.0;       // How much foam boundary moves
const FOAM_TO_COASTAL_BLEND: f32 = 1.0;     // Blend width from foam to coastal
const FOAM_NOISE_STRENGTH: f32 = 0.5;       // How much noise affects foam edge

// Coastal wave animation (medium distance, slower movement)
const COASTAL_ANIM_SPEED: f32 = 0.5;        // How fast coastal waves move
const COASTAL_BASE_DIST: f32 = 0.0;         // Base distance where coastal color starts
const COASTAL_ANIM_AMPLITUDE: f32 = 3.0;    // How much coastal boundary moves
const COASTAL_TO_OCEAN_BLEND: f32 = 400.0;  // Blend width from coastal to deep ocean
const COASTAL_PIXELLATION: f32 = 80.0;      // Noise strength for coastal edge

// --- Water Wave Animation Parameters ---
// Multi-scale noise for realistic wave appearance
const WAVE1_SPEED: f32 = 0.4;
const WAVE1_SCALE: f32 = 0.5;
const WAVE1_WEIGHT: f32 = 1.0;

const WAVE2_SPEED: f32 = 0.2;
const WAVE2_SCALE: f32 = 0.04;
const WAVE2_WEIGHT: f32 = 0.2;

const WAVE3_SPEED: f32 = 0.1;
const WAVE3_SCALE: f32 = 0.001;
const WAVE3_WEIGHT: f32 = 0.4;

const WAVE_COMBINED_SCALE: f32 = 0.5;

// Specular highlights (sun glints)
const SPECULAR_SCALE: f32 = 0.09;
const SPECULAR_STRETCH: vec2<f32> = vec2<f32>(1.0, 2.5);
const SPECULAR_SPEED: f32 = 0.3;
const SPECULAR_POWER: f32 = 32.0;  // Controls tightness of specular highlights

// --- River Detection Parameters ---
// How far to probe for opposite riverbanks (in pixels)
const RIVER_PROBE_DISTANCE: f32 = 1.0;

// Below this combined distance, water is considered a river/narrow channel
const RIVER_WIDTH_THRESHOLD: f32 = RIVER_PROBE_DISTANCE * 2.0;

// Smooth transition width from river to ocean
const RIVER_FADE_WIDTH: f32 = 0.5;

// --- Sphere Projection Parameters ---
// Camera position for sphere rendering
const SPHERE_CAMERA_POS: vec3<f32> = vec3<f32>(0.0, 0.0, -2.5);
const SPHERE_CAMERA_FOV: f32 = 1.5;  // Z component of ray direction, controls FOV
const SPHERE_ROTATION_SPEED: f32 = 0.1;  // How fast the sphere rotates
const SPHERE_HEIGHT_SCALE: f32 = 0.05;  // How much terrain elevation affects sphere displacement
const SPHERE_BASE_RADIUS: f32 = 1.0;  // Base sphere radius before displacement

// --- Player Territory Blending ---
// How much to blend player color with terrain
const PLAYER_COLOR_BLEND: f32 = 0.6;

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
fn is_water(tile: u32) -> bool { return !is_land(tile); }
fn get_magnitude(tile: u32) -> u32 { return tile & 0x1Fu; }

// REMOVED: The expensive distance_to_land function is no longer needed!
// We now use a pre-computed distance field texture instead.

// Calculates a "river factor" from 0.0 (open water/ocean) to 1.0 (narrow channel/river).
// Uses the distance field to detect constriction by checking opposite banks.
fn calculate_river_factor(uv: vec2<f32>) -> f32 {
// --- Recommended Parameters for narrow rivers ---
    const PROBE_DISTANCE: f32 = 3.0;
    const RIVER_MAX_DIST_THRESHOLD: f32 = 1.0;
    const RIVER_FADE_WIDTH: f32 = 8.0;
    // ---

    let pixel_size = 1.0 / texture_size;
    let offset = pixel_size * PROBE_DISTANCE;

    // Sample distance field in four cardinal directions
    let dist_right = textureSample(distance_texture, distance_sampler, uv + vec2<f32>(offset.x, 0.0)).r;
    let dist_left  = textureSample(distance_texture, distance_sampler, uv - vec2<f32>(offset.x, 0.0)).r;
    let dist_up    = textureSample(distance_texture, distance_sampler, uv + vec2<f32>(0.0, offset.y)).r;
    let dist_down  = textureSample(distance_texture, distance_sampler, uv - vec2<f32>(0.0, offset.y)).r;

    // De-normalize distances to pixel space
    let d_r = dist_right * 255.0;
    let d_l = dist_left * 255.0;
    let d_u = dist_up * 255.0;
    let d_d = dist_down * 255.0;

    // --- NEW LOGIC ---
    // Find the furthest distance we can "see" in any of the 4 directions.
    let max_dist_horizontal = max(d_r, d_l);
    let max_dist_vertical = max(d_u, d_d);
    let max_dist = max(max_dist_horizontal, max_dist_vertical);

    // If this max distance is SMALL, it means we are enclosed on all sides -> it's a river.
    // If max_dist is LARGE, it means we found an "escape route" -> it's a coast.
    // We use (1.0 - ...) because a SMALL max_dist means a HIGH river_factor.
    let river_factor = 1.0 - smoothstep(
        RIVER_MAX_DIST_THRESHOLD,
        RIVER_MAX_DIST_THRESHOLD + RIVER_FADE_WIDTH,
        max_dist
    );

    return river_factor;
}

// UPDATED: Now takes UV coordinate for texture sampling instead of discrete pixel coord
fn get_terrain_color(tile: u32, uv: vec2<f32>, coord: vec2<i32>) -> vec4<f32> {
    if (is_land(tile)) {
        let mag = get_magnitude(tile);
        if (mag < 10u) {
            let brightness = 1.0 + (f32(mag) / 10.0) * PLAINS_BRIGHTNESS_SCALE;
            return vec4<f32>(PLAINS_COLOR, 1.0) * brightness;
        } else if (mag < 20u) {
            let brightness = 1.0 + (f32(mag - 10u) / 10.0) * HIGHLAND_BRIGHTNESS_SCALE;
            return vec4<f32>(HIGHLAND_COLOR, 1.0) * brightness;
        } else {
            let brightness = 1.0 + (f32(mag - 20u) / 11.0) * MOUNTAIN_BRIGHTNESS_SCALE;
            return vec4<f32>(MOUNTAIN_COLOR, 1.0) * brightness;
        }
    } else if (is_water(tile)) {
        // Apply same water rendering to both oceans and lakes
        // Sample the distance texture multiple times to blur/smooth the SDF
        // This creates softer, more natural-looking coastal transitions
        let blur_offset = 1.0 / texture_size.x; // One pixel offset in UV space
        let dist_center = textureSample(distance_texture, distance_sampler, uv).r;
        let dist_right = textureSample(distance_texture, distance_sampler, uv + vec2<f32>(blur_offset, 0.0)).r;
        let dist_left = textureSample(distance_texture, distance_sampler, uv - vec2<f32>(blur_offset, 0.0)).r;
        let dist_up = textureSample(distance_texture, distance_sampler, uv + vec2<f32>(0.0, blur_offset)).r;
        let dist_down = textureSample(distance_texture, distance_sampler, uv - vec2<f32>(0.0, blur_offset)).r;

        // Average the samples for a simple box blur
        let dist_blurred = (dist_center + dist_right + dist_left + dist_up + dist_down) / 5.0;
        let dist_raw = dist_blurred * DISTANCE_DENORMALIZE; // Denormalize back to pixels

        // Apply logarithmic falloff for distance-based brightness (brightens distant ocean significantly)
        let dist = dist_raw + FALLOFF_STRENGTH * log(1.0 + dist_raw / FALLOFF_OFFSET);

        // Add noise-based variation to the animation for more organic movement
        let f_coord = vec2<f32>(coord);
        let foam_noise = animated_simplex_noise(f_coord, 1.0, 0.4, time) * 2.0 - 1.0; // Range: -1 to 1
        let coastal_noise = animated_simplex_noise(f_coord, 1.0, 0.4, time) * 2.0 - 1.0; // Range: -1 to 1

        // Animate the distances using sine waves for a smooth ebb and flow, plus noise
        let foam_pulse = (sin(time * FOAM_ANIM_SPEED) + 1.0) * 0.5; // Varies 0.0 to 1.0
        let animated_foam_edge = FOAM_BASE_DIST + foam_pulse * FOAM_ANIM_AMPLITUDE + foam_noise * FOAM_NOISE_STRENGTH;

        // Use a different speed and phase for the coastal wave to make it look more natural
        let coastal_wave = (sin(time * COASTAL_ANIM_SPEED + 2.0) + 1.0) * 0.5;
        let animated_coastal_edge = COASTAL_BASE_DIST + coastal_wave * COASTAL_ANIM_AMPLITUDE + coastal_noise * COASTAL_PIXELLATION;

        // Calculate the mix factors using smoothstep for a nice gradient
        // 1. How much foam should be visible? Fades out from the animated foam edge.
        let foam_mix = 1.0 - smoothstep(animated_foam_edge, animated_foam_edge + FOAM_TO_COASTAL_BLEND, dist_raw);
        // 2. How much coastal water should be visible? Fades out from the animated coastal edge.
        let coastal_mix = 1.0 - smoothstep(animated_coastal_edge, animated_coastal_edge + COASTAL_TO_OCEAN_BLEND, dist);

        // Layer the colors using the mix factors. Start with the outermost color.
        var coastal_base_color = DEEP_OCEAN_COLOR;
        coastal_base_color = mix(coastal_base_color, COASTAL_COLOR, coastal_mix); // Blend coastal on top of deep ocean
        coastal_base_color = mix(coastal_base_color, FOAM_COLOR, foam_mix);       // Blend foam on top of everything

        // --- NEW RIVER DETECTION LOGIC ---
        // Calculate river factor to detect narrow channels
        let river_factor = calculate_river_factor(uv);

        // Blend between complex coastal effects and simple river color
        // river_factor = 1.0 means narrow channel (full river color)
        // river_factor = 0.0 means open ocean (full coastal effects)
        var base_color = mix(coastal_base_color, RIVER_COLOR, river_factor);

        // --- END OF OPTIMIZED LOGIC ---

        // The existing wave animation now applies to our newly blended base color
        // Note: Still uses discrete coord for noise patterns
        if (enable_water_animation != 0u) {
            let f_coord = vec2<f32>(coord);

            let wave1 = animated_simplex_noise(f_coord, WAVE1_SPEED, WAVE1_SCALE, time);
            let wave2 = animated_simplex_noise(f_coord, WAVE2_SPEED, WAVE2_SCALE, time);
            let wave3 = animated_simplex_noise(f_coord, WAVE3_SPEED, WAVE3_SCALE, time);

            let combined_waves = (wave1 * WAVE1_WEIGHT + wave2 * WAVE2_WEIGHT + wave3 * WAVE3_WEIGHT) * WAVE_COMBINED_SCALE;

            // We mix the highlight color with our new base_color (foam, teal, or deep blue)
            var final_color = mix(base_color, WAVE_HIGHLIGHT_COLOR, combined_waves * WAVE_HIGHLIGHT_STRENGTH);

            let specular_noise = animated_simplex_noise(f_coord * SPECULAR_STRETCH, SPECULAR_SPEED, SPECULAR_SCALE, time);

            let glint_amount = pow(specular_noise, SPECULAR_POWER);
            final_color += glint_amount * SPECULAR_COLOR * SPECULAR_STRENGTH;

            return vec4<f32>(final_color, 1.0);
        } else {
            // Simple static water color when animation is disabled
            return vec4<f32>(base_color, 1.0);
        }
    } else {
        // Lake - lighter blue
        return LAKE_COLOR;
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

    if (1u != 0u) {
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
        let time_scaled = time * SPHERE_ROTATION_SPEED;
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

    // UPDATED: Pass both uv (for distance texture sampling) and pixel_coord (for noise)
    var terrain_color = get_terrain_color(center_terrain, uv, pixel_coord);

    // Shoreline Effect
    let center_is_land = is_land(center_terrain);
    if (center_is_land) {
        let top_terrain = get_map_tile_at(pixel_coord + vec2<i32>(0, 1));
        let bottom_terrain = get_map_tile_at(pixel_coord - vec2<i32>(0, 1));
        let left_terrain = get_map_tile_at(pixel_coord - vec2<i32>(1, 0));
        let right_terrain = get_map_tile_at(pixel_coord + vec2<i32>(1, 0));

        let is_shoreline = !is_land(top_terrain) || !is_land(bottom_terrain) || !is_land(left_terrain) || !is_land(right_terrain);

        if (is_shoreline) {
            terrain_color = mix(terrain_color, SAND_COLOR, SAND_BLEND_AMOUNT);
        }
    }

    var base_color: vec4<f32>;
    if (enable_players != 0u) {
        let player_color = player_colors[center_owner];
        if (center_owner == 0u) {
            base_color = terrain_color;
        } else {
            base_color = mix(terrain_color, player_color, PLAYER_COLOR_BLEND);
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

        // Get terrain of neighbors to detect water vs land boundaries
        let top_terrain = get_map_tile_at(pixel_coord + vec2<i32>(0, offset));
        let bottom_terrain = get_map_tile_at(pixel_coord - vec2<i32>(0, offset));
        let left_terrain = get_map_tile_at(pixel_coord - vec2<i32>(offset, 0));
        let right_terrain = get_map_tile_at(pixel_coord + vec2<i32>(offset, 0));

        // Water tiles (oceans and lakes) should NOT create borders at all
        let center_is_water = is_water(center_terrain);

        var is_owner_border = false;
        if (!center_is_water) {
            // Land tiles check for ownership changes OR water neighbors (coastlines)
            // For wilderness (owner 0), we need to check if neighbor is water
            let ownership_change = (center_owner != top_owner || center_owner != bottom_owner || center_owner != left_owner || center_owner != right_owner);

            // If we're wilderness land, also check if any neighbor is water (ocean or lake)
            let has_water_neighbor = (is_water(top_terrain) || is_water(bottom_terrain) || is_water(left_terrain) || is_water(right_terrain));

            if (center_owner == 0u) {
                // Wilderness: border if ownership changes OR if bordering water
                is_owner_border = ownership_change || has_water_neighbor;
            } else {
                // Player territory: border only on ownership changes
                is_owner_border = ownership_change;
            }
        }

        if (is_owner_border) {
            return vec4<f32>(base_color.rgb * border_color.rgb, base_color.a);
        }
    }

    return base_color;
}
