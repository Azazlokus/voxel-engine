//! Блоки, которыми пользуется генератор.

use engine_registry::Identifier;
use engine_world::{BlockId, Blocks};
use thiserror::Error;

/// Не удалось найти блок, нужный генератору.
#[derive(Debug, Error)]
#[error("генератору нужен блок {0}, но его нет в реестре")]
pub struct MissingBlock(pub String);

/// Числовые ID блоков, которыми генератор заполняет мир.
///
/// Разрешаются один раз после заморозки реестра: искать по строке на каждый
/// воксель — это сотни тысяч поисков в хеш-таблице на одну колонку.
#[derive(Debug, Clone, Copy)]
pub struct TerrainPalette {
    /// Камень в глубине.
    pub stone: BlockId,
    /// Земля под поверхностью.
    pub dirt: BlockId,
    /// Трава на поверхности.
    pub grass: BlockId,
    /// Песок у воды.
    pub sand: BlockId,
    /// Вода до уровня моря.
    pub water: BlockId,
    /// Ствол дерева.
    pub log: BlockId,
    /// Листва.
    pub leaves: BlockId,
}

impl TerrainPalette {
    /// Разрешить идентификаторы в числовые ID.
    ///
    /// Генератор требует конкретные блоки `base:*`: он часть базового контента,
    /// а не абстрактный алгоритм. Мод со своим рельефом поставит свой генератор.
    pub fn resolve(blocks: &Blocks) -> Result<Self, MissingBlock> {
        Ok(Self {
            stone: lookup(blocks, "base:stone")?,
            dirt: lookup(blocks, "base:dirt")?,
            grass: lookup(blocks, "base:grass")?,
            sand: lookup(blocks, "base:sand")?,
            water: lookup(blocks, "base:water")?,
            log: lookup(blocks, "base:log")?,
            leaves: lookup(blocks, "base:leaves")?,
        })
    }
}

fn lookup(blocks: &Blocks, name: &str) -> Result<BlockId, MissingBlock> {
    let id = Identifier::parse(name).map_err(|_| MissingBlock(name.to_owned()))?;
    blocks.id(&id).ok_or_else(|| MissingBlock(name.to_owned()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use engine_world::{AIR_ID, BlockProperties, BlockRegistry};

    /// Реестр, собранный вручную: тесты генератора не должны зависеть от
    /// файлов контента на диске.
    pub(crate) fn тестовый_реестр() -> Blocks {
        let mut registry = BlockRegistry::new();
        for name in [
            AIR_ID,
            "base:stone",
            "base:dirt",
            "base:grass",
            "base:sand",
            "base:water",
            "base:log",
            "base:leaves",
        ] {
            let properties = if name == AIR_ID {
                BlockProperties::AIR
            } else {
                BlockProperties::SOLID
            };
            registry
                .register(Identifier::parse(name).expect("корректный id"), properties)
                .expect("блок регистрируется");
        }
        registry.freeze().expect("заморозка удалась")
    }

    /// Палитра из тестового реестра.
    pub(crate) fn тестовая_палитра() -> TerrainPalette {
        TerrainPalette::resolve(&тестовый_реестр()).expect("все блоки на месте")
    }

    #[test]
    fn палитра_разрешается() {
        let palette = тестовая_палитра();
        // Все блоки разные и ни один не воздух.
        let ids = [
            palette.stone,
            palette.dirt,
            palette.grass,
            palette.sand,
            palette.water,
            palette.log,
            palette.leaves,
        ];
        for id in ids {
            assert!(!id.is_air(), "{id:?} не должен быть воздухом");
        }
        let mut unique = ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "ID блоков не должны совпадать");
    }

    #[test]
    fn отсутствие_блока_сообщается() {
        let mut registry = BlockRegistry::new();
        registry
            .register(
                Identifier::parse(AIR_ID).expect("корректный id"),
                BlockProperties::AIR,
            )
            .expect("воздух регистрируется");
        let blocks = registry.freeze().expect("заморозка удалась");

        let err = TerrainPalette::resolve(&blocks).expect_err("камня нет");
        assert!(err.to_string().contains("base:stone"), "{err}");
    }
}
