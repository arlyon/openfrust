pub mod ai;
pub mod plugins;

mod disconnected_fronts;
mod fixed_update_timer;
mod input;
mod map_renderer;
mod player_elimination;
mod player_info;
mod setup;

pub use disconnected_fronts::clear_disconnected_fronts;
pub use fixed_update_timer::{PerfUiEntryGpuTime, setup_gpu_perf_ui};
pub use input::{close_on_esc, handle_render_input};
pub use map_renderer::{
    setup_map_texture, sync_render_settings_to_materials, update_water_animation_time,
};
pub use player_elimination::check_eliminations_and_update_troops;
pub use player_info::update_player_info;
pub use setup::setup;
