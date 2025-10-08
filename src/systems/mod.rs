mod border_calculation;
mod border_material;
mod disconnected_fronts;
mod expansion_assignment;
mod expansion_processing;
mod game_update;
mod map_renderer;
mod player_elimination;
mod player_info;
mod setup;

pub use border_calculation::initial_border_calculation;
pub use border_material::BorderMaterial;
pub use game_update::update_game;
pub use map_renderer::{setup_map_texture, update_map_texture};
pub use player_info::update_player_info;
pub use setup::setup;
