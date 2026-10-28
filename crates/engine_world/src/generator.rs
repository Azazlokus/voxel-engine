//! Трейт генератора мира.

use crate::{Blocks, ChunkPos, Column};
use bevy_ecs::resource::Resource;
use std::fmt;
use std::sync::Arc;

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

/// Активный генератор мира как ресурс.
///
/// Хранится за `Arc`, потому что генерация идёт в пуле потоков: каждой задаче
/// нужна своя ссылка, живущая дольше кадра.
#[derive(Clone, Resource)]
pub struct ActiveGenerator(Arc<dyn WorldGenerator>);

impl ActiveGenerator {
    /// Обернуть генератор.
    pub fn new(generator: Arc<dyn WorldGenerator>) -> Self {
        Self(generator)
    }

    /// Ссылка на генератор для фоновой задачи.
    pub fn get(&self) -> Arc<dyn WorldGenerator> {
        Arc::clone(&self.0)
    }
}

impl fmt::Debug for ActiveGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Трейт не требует Debug от реализаций, а seed — всё, что о генераторе
        // стоит знать в логах.
        f.debug_struct("ActiveGenerator")
            .field("seed", &self.0.seed())
            .finish()
    }
}

impl std::ops::Deref for ActiveGenerator {
    type Target = dyn WorldGenerator;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
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
