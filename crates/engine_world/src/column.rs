//! Колонка чанков: вертикальный столб секций с фиксированным диапазоном по Y.

use crate::{BlockId, ChunkPos, LocalPos, SECTION_SIZE, Section};

/// Нижняя граница мира по Y, включительно.
pub const MIN_Y: i32 = -64;
/// Верхняя граница мира по Y, исключительно.
pub const MAX_Y: i32 = 320;
/// Сколько секций в колонке.
pub const SECTIONS_PER_COLUMN: usize = ((MAX_Y - MIN_Y) / SECTION_SIZE) as usize;
/// Индекс самой нижней секции.
const MIN_SECTION_Y: i32 = MIN_Y / SECTION_SIZE;

/// Колонка чанков 16×384×16.
///
/// Диапазон по Y фиксированный, а не динамический: динамика стоит проверки границ
/// на каждом обращении и ветвления в мешинге, а выигрыш появился бы только у миров
/// с сильно разной высотой, которых у нас нет.
#[derive(Debug, Clone)]
pub struct Column {
    position: ChunkPos,
    sections: Box<[Section; SECTIONS_PER_COLUMN]>,
}

impl Column {
    /// Пустая колонка из воздуха.
    pub fn new(position: ChunkPos) -> Self {
        Self {
            position,
            sections: Box::new(core::array::from_fn(|_| Section::default())),
        }
    }

    /// Позиция колонки.
    #[inline]
    pub fn position(&self) -> ChunkPos {
        self.position
    }

    /// Входит ли координата Y в мир.
    #[inline]
    pub const fn contains_y(y: i32) -> bool {
        y >= MIN_Y && y < MAX_Y
    }

    /// Индекс секции в массиве по мировой координате Y.
    /// `None`, если Y вне мира.
    #[inline]
    pub const fn section_index(y: i32) -> Option<usize> {
        if !Self::contains_y(y) {
            return None;
        }
        // Сдвиг, а не деление: для отрицательных Y деление округляет к нулю.
        Some(((y >> 4) - MIN_SECTION_Y) as usize)
    }

    /// Мировая координата Y нижнего слоя секции.
    #[inline]
    pub const fn section_base_y(index: usize) -> i32 {
        MIN_Y + (index as i32) * SECTION_SIZE
    }

    /// Блок по локальным координатам колонки. За пределами мира — воздух.
    #[inline]
    pub fn get(&self, local_x: u8, y: i32, local_z: u8) -> BlockId {
        match Self::section_index(y) {
            Some(index) => {
                let local = LocalPos::new(local_x, (y & 15) as u8, local_z);
                self.sections[index].get(local)
            }
            None => BlockId::AIR,
        }
    }

    /// Записать блок. Возвращает предыдущий или `None`, если Y вне мира.
    pub fn set(&mut self, local_x: u8, y: i32, local_z: u8, block: BlockId) -> Option<BlockId> {
        let index = Self::section_index(y)?;
        let local = LocalPos::new(local_x, (y & 15) as u8, local_z);
        Some(self.sections[index].set(local, block))
    }

    /// Секция по индексу.
    #[inline]
    pub fn section(&self, index: usize) -> Option<&Section> {
        self.sections.get(index)
    }

    /// Секция по индексу для изменения.
    #[inline]
    pub fn section_mut(&mut self, index: usize) -> Option<&mut Section> {
        self.sections.get_mut(index)
    }

    /// Все секции снизу вверх.
    #[inline]
    pub fn sections(&self) -> &[Section] {
        &self.sections[..]
    }

    /// Все секции снизу вверх для изменения.
    #[inline]
    pub fn sections_mut(&mut self) -> &mut [Section] {
        &mut self.sections[..]
    }

    /// Колонка целиком из воздуха.
    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(Section::is_empty)
    }

    /// Привести все секции к минимальному представлению.
    /// Вызывается один раз после генерации колонки.
    pub fn shrink(&mut self) {
        for section in self.sections.iter_mut() {
            section.shrink();
        }
    }

    /// Сколько байт занимают данные вокселей.
    pub fn data_bytes(&self) -> usize {
        self.sections.iter().map(Section::data_bytes).sum()
    }

    /// Высота верхнего непустого блока, если он есть.
    pub fn highest_block_y(&self) -> Option<i32> {
        for (index, section) in self.sections.iter().enumerate().rev() {
            if section.is_empty() {
                continue;
            }
            let base = Self::section_base_y(index);
            for y in (0..SECTION_SIZE).rev() {
                for z in 0..16u8 {
                    for x in 0..16u8 {
                        if !section.get(LocalPos::new(x, y as u8, z)).is_air() {
                            return Some(base + y);
                        }
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(raw: u16) -> BlockId {
        BlockId::new(raw)
    }

    #[test]
    fn новая_колонка_пуста() {
        let column = Column::new(ChunkPos::new(0, 0));
        assert!(column.is_empty());
        assert_eq!(column.data_bytes(), 0);
        assert_eq!(column.sections().len(), SECTIONS_PER_COLUMN);
        assert_eq!(column.highest_block_y(), None);
    }

    #[test]
    fn диапазон_мира() {
        assert_eq!(SECTIONS_PER_COLUMN, 24);
        assert!(Column::contains_y(MIN_Y));
        assert!(Column::contains_y(MAX_Y - 1));
        assert!(!Column::contains_y(MIN_Y - 1));
        assert!(!Column::contains_y(MAX_Y));
    }

    #[test]
    fn индекс_секции_по_отрицательному_y() {
        // Самая нижняя секция покрывает -64..-49.
        assert_eq!(Column::section_index(-64), Some(0));
        assert_eq!(Column::section_index(-49), Some(0));
        assert_eq!(Column::section_index(-48), Some(1));
        assert_eq!(Column::section_index(-1), Some(3));
        assert_eq!(Column::section_index(0), Some(4));
        assert_eq!(Column::section_index(15), Some(4));
        assert_eq!(Column::section_index(16), Some(5));
        assert_eq!(
            Column::section_index(MAX_Y - 1),
            Some(SECTIONS_PER_COLUMN - 1)
        );
    }

    #[test]
    fn y_вне_мира_не_имеет_секции() {
        assert_eq!(Column::section_index(MIN_Y - 1), None);
        assert_eq!(Column::section_index(MAX_Y), None);
        assert_eq!(Column::section_index(i32::MIN), None);
        assert_eq!(Column::section_index(i32::MAX), None);
    }

    #[test]
    fn индекс_и_базовый_y_обратимы() {
        for index in 0..SECTIONS_PER_COLUMN {
            let base = Column::section_base_y(index);
            assert_eq!(Column::section_index(base), Some(index));
            assert_eq!(Column::section_index(base + 15), Some(index));
        }
    }

    #[test]
    fn запись_и_чтение_включая_отрицательный_y() {
        let mut column = Column::new(ChunkPos::new(-1, -1));
        for y in [MIN_Y, -49, -48, -1, 0, 15, 16, MAX_Y - 1] {
            assert_eq!(column.set(5, y, 9, block(7)), Some(BlockId::AIR));
            assert_eq!(column.get(5, y, 9), block(7), "y={y}");
            // Соседний воксель по Y не затронут.
            if Column::contains_y(y + 1) {
                assert_eq!(column.get(5, y + 1, 9), BlockId::AIR, "y={y}+1");
            }
        }
        assert!(!column.is_empty());
    }

    #[test]
    fn запись_вне_мира_отвергается() {
        let mut column = Column::new(ChunkPos::new(0, 0));
        assert_eq!(column.set(0, MIN_Y - 1, 0, block(1)), None);
        assert_eq!(column.set(0, MAX_Y, 0, block(1)), None);
        assert!(column.is_empty());
    }

    #[test]
    fn чтение_вне_мира_даёт_воздух() {
        let column = Column::new(ChunkPos::new(0, 0));
        assert_eq!(column.get(0, MIN_Y - 1, 0), BlockId::AIR);
        assert_eq!(column.get(0, MAX_Y, 0), BlockId::AIR);
    }

    #[test]
    fn верхний_блок_находится() {
        let mut column = Column::new(ChunkPos::new(0, 0));
        column.set(3, -10, 4, block(1));
        assert_eq!(column.highest_block_y(), Some(-10));
        column.set(3, 70, 4, block(1));
        assert_eq!(column.highest_block_y(), Some(70));
        column.set(3, 70, 4, BlockId::AIR);
        assert_eq!(column.highest_block_y(), Some(-10));
    }

    #[test]
    fn shrink_свёртывает_заполненные_секции() {
        let mut column = Column::new(ChunkPos::new(0, 0));
        for y in MIN_Y..MIN_Y + 16 {
            for z in 0..16u8 {
                for x in 0..16u8 {
                    column.set(x, y, z, block(1));
                }
            }
        }
        assert!(column.data_bytes() > 0);
        column.shrink();
        assert_eq!(
            column.data_bytes(),
            0,
            "однородные секции не должны хранить воксели"
        );
        assert_eq!(column.get(7, MIN_Y + 5, 7), block(1));
    }

    #[test]
    fn позиция_сохраняется() {
        let column = Column::new(ChunkPos::new(-3, 5));
        assert_eq!(column.position(), ChunkPos::new(-3, 5));
    }
}
