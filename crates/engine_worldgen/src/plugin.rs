//! Плагин генерации мира.

use crate::TerrainGenerator;
use engine_core::{App, Plugin};
use engine_world::ActiveGenerator;
use std::sync::Arc;

/// Плагин генерации ландшафта.
///
/// Ставит [`TerrainGenerator`] активным генератором мира. Мод со своим рельефом
/// заменит его, поставив свой плагин вместо этого.
#[derive(Debug, Clone, Copy)]
pub struct WorldgenPlugin {
    seed: u64,
}

impl WorldgenPlugin {
    /// Плагин с заданным seed.
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Seed генератора.
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl Default for WorldgenPlugin {
    /// Seed по умолчанию. Фиксированный, а не случайный: мир должен быть
    /// одинаковым между запусками, пока не попросили иначе.
    fn default() -> Self {
        Self::new(0x5eed)
    }
}

impl Plugin for WorldgenPlugin {
    fn name(&self) -> &'static str {
        "engine_worldgen"
    }

    fn build(&self, app: &mut App) {
        app.insert_resource(ActiveGenerator::new(Arc::new(TerrainGenerator::new(
            self.seed,
        ))));
    }
}
