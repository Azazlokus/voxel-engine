//! Трейт генератора мира.

use crate::{Blocks, ChunkPos, Column};

/// Источник содержимого колонок.
///
/// `Send + Sync`, потому что генерация идёт в пуле потоков: главный поток только
/// отдаёт задачи и принимает готовые колонки.
///
/// Реализация обязана быть **детерминированной**: одна и та же позиция при одном
/// seed даёт одну и ту же колонку, сколько бы раз её ни сгенерировали и в каком бы
/// порядке. Иначе чанк, выгруженный и загруженный заново, не совпадёт с соседями,
/// и на стыке появится шов.
pub trait WorldGenerator: Send + Sync + 'static {
    /// Заполнить колонку. Колонка приходит пустой (из воздуха).
    fn generate_column(&self, position: ChunkPos, blocks: &Blocks, out: &mut Column);

    /// Seed генератора. Для логов и диагностики.
    fn seed(&self) -> u64;
}

/// Генератор, не создающий ничего. Нужен, чтобы собрать мир без `engine_worldgen`
/// (тесты, headless-прогон).
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyGenerator;

impl WorldGenerator for EmptyGenerator {
    fn generate_column(&self, _position: ChunkPos, _blocks: &Blocks, _out: &mut Column) {}

    fn seed(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockProperties, BlockRegistry};
    use engine_registry::Identifier;

    fn пустой_реестр() -> Blocks {
        let mut registry = BlockRegistry::new();
        registry
            .register(
                Identifier::parse(crate::AIR_ID).expect("корректный id"),
                BlockProperties::AIR,
            )
            .expect("воздух регистрируется");
        registry.freeze().expect("заморозка удалась")
    }

    #[test]
    fn пустой_генератор_оставляет_колонку_пустой() {
        let blocks = пустой_реестр();
        let mut column = Column::new(ChunkPos::new(0, 0));
        EmptyGenerator.generate_column(ChunkPos::new(0, 0), &blocks, &mut column);
        assert!(column.is_empty());
        assert_eq!(EmptyGenerator.seed(), 0);
    }
}
