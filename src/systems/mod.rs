mod border_material;
mod cursor_query;
mod disconnected_fronts;
mod expansion_assignment;
mod gpu;
mod gpu_orchestrator;
mod map_renderer;
mod perf_ui;
mod player_elimination;
mod player_info;
mod setup;
mod sim_manager;

pub use border_material::BorderMaterial;
pub use cursor_query::{
    dispatch_id_query, process_id_query_result, setup_player_info_panel,
    update_cursor_query, update_player_info_panel, CursorIDQuery, CursorIDResult,
};
pub use gpu::{ExpansionWorker, PlayerIdWorker};
pub use gpu_orchestrator::gpu_read;
pub use map_renderer::setup_map_texture;
pub use perf_ui::{PerfUiEntryGpuTime, setup_gpu_perf_ui};
pub use player_info::update_player_info;
pub use setup::setup;
pub use sim_manager::SimManager;
