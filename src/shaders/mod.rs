mod border_material;
pub mod compute;
mod frame_manager;

pub use frame_manager::GpuFrameManager;

pub use border_material::BorderMaterial;

use bevy::{reflect::TypePath, shader::ShaderRef};
use bevy_app_compute::prelude::ComputeShader;

/// Shader definition for clearing atomic buffers
#[derive(TypePath)]
pub struct ClearBuffersShader;

impl ComputeShader for ClearBuffersShader {
    fn shader() -> ShaderRef {
        "shaders/clear_buffers.wgsl".into()
    }
}

/// Shader definition for the expansion compute pass
#[derive(TypePath)]
pub struct ExpansionShader;

impl ComputeShader for ExpansionShader {
    fn shader() -> ShaderRef {
        "shaders/expansion.wgsl".into()
    }
}

/// Shader definition for the results processing pass (reduction only)
#[derive(TypePath)]
pub struct ProcessResultsShader;

impl ComputeShader for ProcessResultsShader {
    fn shader() -> ShaderRef {
        "shaders/process_results.wgsl".into()
    }
}

/// Shader definition for the border adjacency calculation pass
#[derive(TypePath)]
pub struct BorderAdjacencyShader;

impl ComputeShader for BorderAdjacencyShader {
    fn shader() -> ShaderRef {
        "shaders/border_adjacency.wgsl".into()
    }
}

#[derive(TypePath)]
pub struct GetPlayerIdShader;

impl ComputeShader for GetPlayerIdShader {
    fn shader() -> ShaderRef {
        "shaders/get_player_id.wgsl".into()
    }
}
