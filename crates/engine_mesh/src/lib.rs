//! Greedy meshing секций воксельного мира.
//!
//! Крейт не знает ни про GPU, ни про мир целиком: на вход снимок секции с полем
//! соседей, на выход вершины и индексы. Поэтому мешинг тестируется и
//! бенчмаркается без окна и без видеокарты.

mod greedy;
mod vertex;
mod view;

pub use greedy::{MeshData, TextureLookup, greedy_mesh};
pub use vertex::{AO_MAX, MAX_POS, MAX_QUAD_SIZE, MAX_TEXTURE, PackedVertex};
pub use view::{PADDED_SIZE, PADDED_VOLUME, PaddedSection};
