use bevy::ecs::system::{Res, SystemParam};
use bevy::prelude::*;
use iyes_perf_ui::entry::PerfUiEntry;
use iyes_perf_ui::prelude::*;

use super::GpuOrchestratorTime;

/// Spawn the GPU timing UI in the top left corner
pub fn setup_gpu_perf_ui(mut commands: Commands) {
    commands.spawn((
        PerfUiRoot {
            position: PerfUiPosition::TopLeft,
            display_labels: true,
            ..default()
        },
        PerfUiEntryGpuTime,
    ));
}

/// Custom perf UI entry for GPU orchestrator time
#[derive(Component, Default)]
pub struct PerfUiEntryGpuTime;

impl PerfUiEntry for PerfUiEntryGpuTime {
    type SystemParam = Res<'static, GpuOrchestratorTime>;
    type Value = f64;

    fn label(&self) -> &'static str {
        "Fixed Update GPU"
    }

    fn sort_key(&self) -> i32 {
        0
    }

    fn update_value(
        &self,
        timing: &mut <Self::SystemParam as SystemParam>::Item<'_, '_>,
    ) -> Option<Self::Value> {
        Some(timing.get())
    }

    fn format_value(&self, value: &Self::Value) -> String {
        format!("{value:.2} ms")
    }
}
