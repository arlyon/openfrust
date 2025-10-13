pub mod ai;
pub mod plugins;

mod disconnected_fronts;
mod fixed_update_timer;
mod map_renderer;
mod player_elimination;
mod player_info;
mod setup;

pub use disconnected_fronts::clear_disconnected_fronts;
pub use fixed_update_timer::{PerfUiEntryGpuTime, setup_gpu_perf_ui};
pub use map_renderer::setup_map_texture;
pub use player_elimination::check_eliminations_and_update_troops;
pub use player_info::update_player_info;
pub use setup::setup;
