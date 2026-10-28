//! Загрузка описаний контента из `assets/<namespace>/`.
//!
//! Контент описывается данными, а не кодом: новый блок — это файл RON и текстура,
//! без правок движка. Код нужен только для поведения.

mod block_def;
pub(crate) mod loader;
mod plugin;

pub use block_def::{BlockDefinition, TextureSet};
pub use loader::{AssetError, ContentRoot, LoadedBlock, load_block, load_blocks};
pub use plugin::AssetsPlugin;
