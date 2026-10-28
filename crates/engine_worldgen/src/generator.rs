//! Генератор ландшафта.

use crate::palette::TerrainPalette;
use crate::terrain::{HeightMap, fill_terrain};
use crate::trees::place_trees;
use engine_world::{Blocks, ChunkPos, Column, WorldGenerator};
use std::sync::OnceLock;
use tracing::error;

/// Генератор ландшафта на шуме.
///
/// Детерминирован по seed: координаты колонки однозначно определяют её содержимое,
/// а деревья видны одинаково из всех соседних колонок (см. `trees`).
#[derive(Debug)]
pub struct TerrainGenerator {
    seed: u64,
    heights: HeightMap,
    /// Палитра разрешается при первом вызове: до заморозки реестра числовых ID
    /// ещё не существует, а трейт не даёт отдельного шага инициализации.
    /// `None` внутри означает, что нужных блоков нет — ошибка логируется один раз.
    palette: OnceLock<Option<TerrainPalette>>,
}

impl TerrainGenerator {
    /// Генератор для seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            heights: HeightMap::new(seed),
            palette: OnceLock::new(),
        }
    }

    /// Карта высот — для рендера, отладки и стриминга.
    pub fn heights(&self) -> &HeightMap {
        &self.heights
    }

    fn palette(&self, blocks: &Blocks) -> Option<TerrainPalette> {
        *self
            .palette
            .get_or_init(|| match TerrainPalette::resolve(blocks) {
                Ok(palette) => Some(palette),
                Err(err) => {
                    error!(%err, "генератор не может работать без базовых блоков");
                    None
                }
            })
    }
}

impl WorldGenerator for TerrainGenerator {
    fn generate_column(&self, position: ChunkPos, blocks: &Blocks, out: &mut Column) {
        let Some(palette) = self.palette(blocks) else {
            // Ошибка уже залогирована: возвращаем пустую колонку, а не паникуем
            // в фоновом потоке.
            return;
        };
        debug_assert_eq!(position, out.position(), "колонка не на своём месте");

        fill_terrain(&self.heights, out, &palette);
        place_trees(self.seed, &self.heights, out, &palette);

        // Одно сжатие после всей записи, а не после каждого блока: приводит
        // однородные секции к двум байтам и ужимает палитры до нужной ширины.
        out.shrink();
    }

    fn seed(&self) -> u64 {
        self.seed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::tests::тестовый_реестр;
    use engine_world::{MIN_Y, SECTIONS_PER_COLUMN};

    fn сгенерировать(seed: u64, position: ChunkPos) -> Column {
        let blocks = тестовый_реестр();
        let generator = TerrainGenerator::new(seed);
        let mut column = Column::new(position);
        generator.generate_column(position, &blocks, &mut column);
        column
    }

    #[test]
    fn колонка_не_пуста() {
        let column = сгенерировать(1, ChunkPos::new(0, 0));
        assert!(!column.is_empty());
        assert!(column.highest_block_y().is_some());
    }

    #[test]
    fn генерация_детерминирована() {
        for position in [ChunkPos::new(0, 0), ChunkPos::new(-5, 11)] {
            let a = сгенерировать(777, position);
            let b = сгенерировать(777, position);
            for y in MIN_Y..200 {
                for x in 0..16u8 {
                    for z in 0..16u8 {
                        assert_eq!(
                            a.get(x, y, z),
                            b.get(x, y, z),
                            "({x}, {y}, {z}) в {position:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn разные_seed_дают_разные_колонки() {
        let a = сгенерировать(1, ChunkPos::new(0, 0));
        let b = сгенерировать(2, ChunkPos::new(0, 0));
        let differences = (MIN_Y..150)
            .flat_map(|y| (0..16u8).map(move |x| (x, y)))
            .filter(|(x, y)| a.get(*x, *y, 0) != b.get(*x, *y, 0))
            .count();
        assert!(differences > 0, "колонки совпали при разных seed");
    }

    #[test]
    fn отрицательные_координаты_работают() {
        for position in [
            ChunkPos::new(-1, -1),
            ChunkPos::new(-1, 0),
            ChunkPos::new(0, -1),
            ChunkPos::new(-100, -100),
        ] {
            let column = сгенерировать(42, position);
            assert!(!column.is_empty(), "колонка {position:?} пуста");
            assert_eq!(column.position(), position);
        }
    }

    #[test]
    fn сжатие_оставляет_однородные_секции_дешёвыми() {
        let column = сгенерировать(3, ChunkPos::new(0, 0));
        // Верхние секции — чистый воздух, нижние — сплошной камень:
        // и те и другие обязаны свернуться в Uniform.
        let однородных = (0..SECTIONS_PER_COLUMN)
            .filter(|index| {
                column
                    .section(*index)
                    .and_then(|section| section.uniform_block())
                    .is_some()
            })
            .count();
        assert!(
            однородных >= SECTIONS_PER_COLUMN / 2,
            "однородных секций всего {однородных} из {SECTIONS_PER_COLUMN}"
        );
    }

    #[test]
    fn без_нужных_блоков_колонка_остаётся_пустой() {
        use engine_registry::Identifier;
        use engine_world::{AIR_ID, BlockProperties, BlockRegistry};

        let mut registry = BlockRegistry::new();
        registry
            .register(
                Identifier::parse(AIR_ID).expect("корректный id"),
                BlockProperties::AIR,
            )
            .expect("воздух регистрируется");
        let blocks = registry.freeze().expect("заморозка удалась");

        let generator = TerrainGenerator::new(1);
        let mut column = Column::new(ChunkPos::new(0, 0));
        // Не паника: фоновый поток не должен валить процесс из-за контента.
        generator.generate_column(ChunkPos::new(0, 0), &blocks, &mut column);
        assert!(column.is_empty());
    }

    #[test]
    fn seed_доступен() {
        let generator = TerrainGenerator::new(12345);
        assert_eq!(generator.seed(), 12345);
    }
}
