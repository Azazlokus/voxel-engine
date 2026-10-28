//! Воксельный мир: координаты, блоки, секции с палитрой, колонки чанков.
//!
//! Крейт не зависит ни от `wgpu`, ни от `winit`: всё здесь тестируется и
//! бенчмаркается без GPU и без окна.

mod block;
mod column;
mod coords;
mod face;
mod generator;
mod plugin;
mod section;
mod world;

pub use block::{AIR_ID, BlockId, BlockProperties, BlockRegistry, BlockRegistryError, Blocks};
pub use column::{Column, MAX_Y, MIN_Y, SECTIONS_PER_COLUMN};
pub use coords::{BlockPos, ChunkPos, LocalPos, SECTION_SIZE, SECTION_VOLUME};
pub use face::Face;
pub use generator::{ActiveGenerator, EmptyGenerator, WorldGenerator};
pub use plugin::{BlocksResource, ContentSet, WorldPlugin};
pub use section::Section;
pub use world::VoxelWorld;
