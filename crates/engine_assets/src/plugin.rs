//! Плагин ассетов: корень контента как ресурс.

use crate::ContentRoot;
use engine_core::{App, Plugin};

/// Плагин ассетов.
///
/// Даёт [`ContentRoot`] — папку, из которой плагины контента читают свои описания.
/// Путь берётся из переменной среды `VOXEL_ASSETS`, иначе это `assets` рядом с
/// рабочим каталогом.
#[derive(Debug, Clone, Default)]
pub struct AssetsPlugin {
    root: Option<ContentRoot>,
}

impl AssetsPlugin {
    /// Плагин с корнем контента по умолчанию.
    pub fn new() -> Self {
        Self::default()
    }

    /// Плагин с явно заданным корнем контента.
    pub fn with_root(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            root: Some(ContentRoot::new(root)),
        }
    }
}

impl Plugin for AssetsPlugin {
    fn name(&self) -> &'static str {
        "engine_assets"
    }

    fn build(&self, app: &mut App) {
        let root = self
            .root
            .clone()
            .unwrap_or_else(|| match std::env::var_os("VOXEL_ASSETS") {
                Some(path) => ContentRoot::new(path),
                None => ContentRoot::default(),
            });
        app.insert_resource(root);
    }
}
