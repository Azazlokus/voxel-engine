//! Генератор ландшафта: рельеф на шуме, слои блоков, вода и деревья.

mod generator;
mod noise;
mod palette;
mod plugin;
mod terrain;
mod trees;

pub use generator::TerrainGenerator;
pub use noise::Fbm;
pub use palette::{MissingBlock, TerrainPalette};
pub use plugin::WorldgenPlugin;
pub use terrain::{AMPLITUDE, BASE_HEIGHT, HeightMap, SEA_LEVEL};
