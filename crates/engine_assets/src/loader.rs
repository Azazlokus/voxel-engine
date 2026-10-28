//! Чтение описаний контента с диска.

use crate::BlockDefinition;
use bevy_ecs::resource::Resource;
use engine_registry::{Identifier, IdentifierError};
use ron::extensions::Extensions;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

/// Корень контента: папка, внутри которой лежат пространства имён.
///
/// Путь определяется при запуске (рядом с бинарником или из переменной среды),
/// поэтому это ресурс, а не константа.
#[derive(Debug, Clone, Resource)]
pub struct ContentRoot(PathBuf);

impl ContentRoot {
    /// Корень контента по пути.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Путь к корню.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Папка пространства имён.
    pub fn namespace_dir(&self, namespace: &str) -> PathBuf {
        self.0.join(namespace)
    }

    /// Папка с описаниями блоков пространства имён.
    pub fn blocks_dir(&self, namespace: &str) -> PathBuf {
        self.namespace_dir(namespace).join("blocks")
    }

    /// Папка с текстурами пространства имён.
    pub fn textures_dir(&self, namespace: &str) -> PathBuf {
        self.namespace_dir(namespace).join("textures")
    }
}

impl Default for ContentRoot {
    /// По умолчанию — папка `assets` рядом с текущим рабочим каталогом.
    fn default() -> Self {
        Self::new("assets")
    }
}

/// Что помешало прочитать контент.
///
/// Каждая ошибка несёт путь: без него сообщение «ожидался `)`» бесполезно,
/// когда файлов десятки.
#[derive(Debug, Error)]
pub enum AssetError {
    /// Папка или файл недоступны.
    #[error("не удалось прочитать {path}: {source}")]
    Io {
        /// Путь, на котором споткнулись.
        path: PathBuf,
        /// Исходная ошибка.
        source: io::Error,
    },

    /// Файл не разобрался как RON.
    ///
    /// Ошибка разбора лежит в боксе: `SpannedError` велик, а он попал бы в размер
    /// каждого `Result` загрузчика, включая успешные возвраты.
    #[error("не удалось разобрать {path}: {source}")]
    Ron {
        /// Путь к файлу.
        path: PathBuf,
        /// Исходная ошибка.
        source: Box<ron::error::SpannedError>,
    },

    /// Имя файла не годится в путь идентификатора.
    #[error("имя файла {path} не годится в идентификатор: {source}")]
    BadFileName {
        /// Путь к файлу.
        path: PathBuf,
        /// Чем именно не годится.
        source: IdentifierError,
    },

    /// У непустого блока нет текстур.
    #[error("в {path} у непустого блока не указаны текстуры")]
    MissingTextures {
        /// Путь к файлу.
        path: PathBuf,
    },

    /// Текстура в описании указана неверно.
    #[error("в {path} текстура {texture:?} не является идентификатором: {source}")]
    BadTexture {
        /// Путь к файлу.
        path: PathBuf,
        /// Текст, который не разобрался.
        texture: String,
        /// Чем именно не годится.
        source: IdentifierError,
    },
}

/// Описание блока вместе с его идентификатором и путём к файлу.
#[derive(Debug, Clone)]
pub struct LoadedBlock {
    /// Идентификатор: пространство имён плюс имя файла без расширения.
    pub id: Identifier,
    /// Разобранное описание.
    pub definition: BlockDefinition,
    /// Откуда прочитано — для сообщений об ошибках.
    pub source: PathBuf,
}

/// Прочитать все описания блоков пространства имён.
///
/// Файлы читаются в отсортированном порядке, чтобы ошибки и логи не зависели
/// от порядка, в котором их отдаёт файловая система.
///
/// Отсутствие папки `blocks` — не ошибка: пространство имён может содержать
/// только текстуры.
pub fn load_blocks(root: &ContentRoot, namespace: &str) -> Result<Vec<LoadedBlock>, AssetError> {
    let dir = root.blocks_dir(namespace);
    if !dir.is_dir() {
        debug!(dir = %dir.display(), "папки с описаниями блоков нет, пропускаю");
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|source| AssetError::Io {
        path: dir.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AssetError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut blocks = Vec::with_capacity(paths.len());
    for path in paths {
        blocks.push(load_block(namespace, &path)?);
    }
    debug!(
        namespace,
        blocks = blocks.len(),
        "описания блоков прочитаны"
    );
    Ok(blocks)
}

/// Настройки разбора RON для описаний контента.
///
/// `implicit_some` позволяет писать `textures: All("base:stone")` вместо
/// `textures: Some(All("base:stone"))`: обёртка `Some` — деталь представления
/// в Rust, автору контента она ничего не сообщает.
pub(crate) fn ron_options() -> ron::Options {
    ron::Options::default().with_default_extension(Extensions::IMPLICIT_SOME)
}

/// Прочитать одно описание блока.
pub fn load_block(namespace: &str, path: &Path) -> Result<LoadedBlock, AssetError> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let id = Identifier::new(namespace, stem).map_err(|source| AssetError::BadFileName {
        path: path.to_path_buf(),
        source,
    })?;

    let text = std::fs::read_to_string(path).map_err(|source| AssetError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let definition: BlockDefinition =
        ron_options()
            .from_str(&text)
            .map_err(|source| AssetError::Ron {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?;

    if definition.needs_textures() && definition.textures.is_none() {
        return Err(AssetError::MissingTextures {
            path: path.to_path_buf(),
        });
    }

    // Текстуры проверяются здесь, а не при использовании: ошибка в описании
    // должна всплыть на загрузке, с именем файла, а не позже в рендере.
    for texture in definition.textures() {
        Identifier::parse(texture).map_err(|source| AssetError::BadTexture {
            path: path.to_path_buf(),
            texture: texture.to_owned(),
            source,
        })?;
    }

    Ok(LoadedBlock {
        id,
        definition,
        source: path.to_path_buf(),
    })
}
