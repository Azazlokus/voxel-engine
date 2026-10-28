//! Мир: загруженные колонки чанков и доступ к блокам по мировым координатам.

use crate::{BlockId, BlockPos, ChunkPos, Column};
use bevy_ecs::resource::Resource;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// Загруженная часть мира.
///
/// Колонки изменяются только в главном потоке. Фоновые задачи генерации и мешинга
/// получают данные копией (снимком), а не ссылкой: иначе понадобилась бы блокировка
/// на каждое чтение вокселя, то есть в самом горячем месте мешера.
#[derive(Debug, Default, Resource)]
pub struct VoxelWorld {
    columns: HashMap<ChunkPos, Column>,
}

impl VoxelWorld {
    /// Пустой мир.
    pub fn new() -> Self {
        Self::default()
    }

    /// Сколько колонок загружено.
    #[inline]
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Загружена ли хоть одна колонка.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Загружена ли колонка.
    #[inline]
    pub fn contains(&self, position: ChunkPos) -> bool {
        self.columns.contains_key(&position)
    }

    /// Колонка, если загружена.
    #[inline]
    pub fn column(&self, position: ChunkPos) -> Option<&Column> {
        self.columns.get(&position)
    }

    /// Колонка для изменения, если загружена.
    #[inline]
    pub fn column_mut(&mut self, position: ChunkPos) -> Option<&mut Column> {
        self.columns.get_mut(&position)
    }

    /// Вставить колонку. Возвращает предыдущую, если на этом месте уже была.
    pub fn insert(&mut self, column: Column) -> Option<Column> {
        self.columns.insert(column.position(), column)
    }

    /// Вставить колонку, только если её ещё нет.
    ///
    /// Возвращает `false`, если колонка уже была: фоновая генерация может прийти
    /// с опозданием, когда игрок успел вернуться и колонка уже загружена, и тогда
    /// готовый результат нужно отбросить, а не затирать живые данные.
    pub fn insert_if_absent(&mut self, column: Column) -> bool {
        match self.columns.entry(column.position()) {
            Entry::Occupied(_) => false,
            Entry::Vacant(vacant) => {
                vacant.insert(column);
                true
            }
        }
    }

    /// Выгрузить колонку.
    pub fn remove(&mut self, position: ChunkPos) -> Option<Column> {
        self.columns.remove(&position)
    }

    /// Позиции всех загруженных колонок.
    pub fn positions(&self) -> impl ExactSizeIterator<Item = ChunkPos> + '_ {
        self.columns.keys().copied()
    }

    /// Все загруженные колонки.
    pub fn columns(&self) -> impl ExactSizeIterator<Item = &Column> + '_ {
        self.columns.values()
    }

    /// Блок по мировой позиции. Для незагруженной колонки или Y вне мира — воздух.
    #[inline]
    pub fn block(&self, pos: BlockPos) -> BlockId {
        match self.columns.get(&pos.chunk()) {
            Some(column) => {
                let local = pos.local();
                column.get(local.x(), pos.y, local.z())
            }
            None => BlockId::AIR,
        }
    }

    /// Записать блок. Возвращает предыдущий, либо `None`, если колонка не загружена
    /// или Y вне мира.
    pub fn set_block(&mut self, pos: BlockPos, block: BlockId) -> Option<BlockId> {
        let column = self.columns.get_mut(&pos.chunk())?;
        let local = pos.local();
        column.set(local.x(), pos.y, local.z(), block)
    }

    /// Сколько байт занимают воксели всех загруженных колонок.
    pub fn data_bytes(&self) -> usize {
        self.columns.values().map(Column::data_bytes).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::{MAX_Y, MIN_Y};

    fn block(raw: u16) -> BlockId {
        BlockId::new(raw)
    }

    #[test]
    fn новый_мир_пуст() {
        let world = VoxelWorld::new();
        assert!(world.is_empty());
        assert_eq!(world.len(), 0);
        assert_eq!(world.data_bytes(), 0);
    }

    #[test]
    fn вставка_и_поиск_колонки() {
        let mut world = VoxelWorld::new();
        let position = ChunkPos::new(-2, 3);
        assert!(world.insert(Column::new(position)).is_none());
        assert!(world.contains(position));
        assert_eq!(world.len(), 1);
        assert!(world.column(position).is_some());
        assert!(world.column(ChunkPos::new(0, 0)).is_none());
    }

    #[test]
    fn повторная_вставка_возвращает_прежнюю_колонку() {
        let mut world = VoxelWorld::new();
        let position = ChunkPos::new(0, 0);
        world.insert(Column::new(position));
        assert!(
            world.insert(Column::new(position)).is_some(),
            "прежняя колонка должна вернуться вызывающему"
        );
        assert_eq!(world.len(), 1);
    }

    #[test]
    fn вставка_только_при_отсутствии_не_затирает_данные() {
        let mut world = VoxelWorld::new();
        let position = ChunkPos::new(1, 1);
        let mut column = Column::new(position);
        column.set(0, 0, 0, block(5));
        world.insert(column);

        // Опоздавшая фоновая генерация не должна затереть живые данные.
        assert!(!world.insert_if_absent(Column::new(position)));
        assert_eq!(world.block(position.origin(0)), block(5));

        assert!(world.insert_if_absent(Column::new(ChunkPos::new(2, 2))));
    }

    #[test]
    fn выгрузка_колонки() {
        let mut world = VoxelWorld::new();
        let position = ChunkPos::new(4, -4);
        world.insert(Column::new(position));
        assert!(world.remove(position).is_some());
        assert!(!world.contains(position));
        assert!(world.remove(position).is_none());
    }

    #[test]
    fn блок_по_мировым_координатам_в_отрицательной_четверти() {
        let mut world = VoxelWorld::new();
        let position = ChunkPos::new(-1, -1);
        world.insert(Column::new(position));

        // Блок (-1, 0, -1) лежит в колонке (-1, -1) с локальными (15, 0, 15).
        let pos = BlockPos::new(-1, 0, -1);
        assert_eq!(pos.chunk(), position);
        assert_eq!(world.set_block(pos, block(3)), Some(BlockId::AIR));
        assert_eq!(world.block(pos), block(3));

        // Блок (0, 0, 0) — уже другая колонка, которой нет: воздух.
        assert_eq!(world.block(BlockPos::new(0, 0, 0)), BlockId::AIR);
    }

    #[test]
    fn швов_на_осях_нет() {
        // Четыре колонки вокруг начала координат: каждый блок должен попасть
        // в свою колонку, и ни один не должен потеряться.
        let mut world = VoxelWorld::new();
        for (cx, cz) in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
            world.insert(Column::new(ChunkPos::new(cx, cz)));
        }
        for x in -16..16 {
            for z in -16..16 {
                let pos = BlockPos::new(x, 0, z);
                let marker = block(((x + 16) * 32 + (z + 16) + 1) as u16);
                assert_eq!(
                    world.set_block(pos, marker),
                    Some(BlockId::AIR),
                    "блок ({x}, 0, {z}) должен попасть в загруженную колонку"
                );
            }
        }
        for x in -16..16 {
            for z in -16..16 {
                let marker = block(((x + 16) * 32 + (z + 16) + 1) as u16);
                assert_eq!(world.block(BlockPos::new(x, 0, z)), marker, "({x}, 0, {z})");
            }
        }
    }

    #[test]
    fn запись_в_незагруженную_колонку_отвергается() {
        let mut world = VoxelWorld::new();
        assert_eq!(world.set_block(BlockPos::new(0, 0, 0), block(1)), None);
    }

    #[test]
    fn запись_вне_диапазона_y_отвергается() {
        let mut world = VoxelWorld::new();
        world.insert(Column::new(ChunkPos::new(0, 0)));
        assert_eq!(world.set_block(BlockPos::new(0, MAX_Y, 0), block(1)), None);
        assert_eq!(
            world.set_block(BlockPos::new(0, MIN_Y - 1, 0), block(1)),
            None
        );
        assert_eq!(world.block(BlockPos::new(0, MAX_Y, 0)), BlockId::AIR);
    }
}
