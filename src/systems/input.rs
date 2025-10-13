use bevy::prelude::*;

use crate::types::RenderSettings;

/// Handles keyboard input to toggle rendering settings
/// - R key: toggles water animation on/off
/// - Space key: disables player rendering (colors and borders) while held
pub fn handle_render_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut render_settings: ResMut<RenderSettings>,
) {
    // Toggle water animation with R key (on press, not held)
    if keyboard.just_pressed(KeyCode::KeyR) {
        render_settings.enable_water_animation = !render_settings.enable_water_animation;
        info!(
            "Water animation: {}",
            if render_settings.enable_water_animation { "ON" } else { "OFF" }
        );
    }

    // Disable player rendering while Space is held
    let space_held = keyboard.pressed(KeyCode::Space);
    if space_held != !render_settings.enable_players {
        render_settings.enable_players = !space_held;
        if !render_settings.enable_players {
            info!("Player rendering: OFF (Space held)");
        } else {
            info!("Player rendering: ON (Space released)");
        }
    }
}
