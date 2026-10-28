//! Снимок секции с полем соседей.

use engine_world::{BlockId, BlockPos, ChunkPos, Column, SECTION_SIZE, VoxelWorld};

/// Сторона снимка: секция 16³ плюс слой соседей с каждой стороны.
pub const PADDED_SIZE: usize = 18;
/// Сколько вокселей в снимке.
pub const PADDED_VOLUME: usize = PADDED_SIZE * PADDED_SIZE * PADDED_SIZE;

/// Секция вместе с граничным слоем соседей.
///
/// Мешер работает с копией, а не со ссылками на мир, по двум причинам. Во-первых,
/// задача уходит в пул потоков и не должна держать блокировку на мире, пока
/// главный поток хочет его менять. Во-вторых, обращение к соседней секции через
/// мир — это поиск колонки в хеш-таблице на каждый воксель границы; здесь же
/// любое чтение это индексация массива.
///
/// 18³ × 2 байта = 11.6 КиБ, буфер переиспользуется между задачами.
#[derive(Debug, Clone)]
pub struct PaddedSection {
    blocks: Box<[BlockId; PADDED_VOLUME]>,
    position: ChunkPos,
    section_index: usize,
    base_y: i32,
    /// Сколько непустых блоков в самой секции, без поля соседей.
    ///
    /// Считается при заполнении: иначе проверка «есть ли что мешить» стоила бы
    /// прохода по 4096 вокселям на каждую из 24 секций колонки.
    non_air: u32,
}

impl PaddedSection {
    /// Пустой снимок.
    pub fn new() -> Self {
        Self {
            blocks: Box::new([BlockId::AIR; PADDED_VOLUME]),
            position: ChunkPos::new(0, 0),
            section_index: 0,
            base_y: 0,
            non_air: 0,
        }
    }

    /// Позиция колонки, из которой снят снимок.
    #[inline]
    pub fn position(&self) -> ChunkPos {
        self.position
    }

    /// Индекс секции в колонке.
    #[inline]
    pub fn section_index(&self) -> usize {
        self.section_index
    }

    /// Мировая Y нижнего слоя секции.
    #[inline]
    pub fn base_y(&self) -> i32 {
        self.base_y
    }

    /// Блок по координатам `-1..=16` относительно секции.
    ///
    /// # Паника
    ///
    /// Если координата вне `-1..=16`.
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> BlockId {
        self.blocks[Self::index(x, y, z)]
    }

    /// Записать блок. Для тестов и ручной сборки снимка.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, block: BlockId) {
        let index = Self::index(x, y, z);
        if Self::in_core(x, y, z) {
            let previous = self.blocks[index];
            match (previous.is_air(), block.is_air()) {
                (true, false) => self.non_air += 1,
                (false, true) => self.non_air -= 1,
                _ => {}
            }
        }
        self.blocks[index] = block;
    }

    /// Лежит ли координата в самой секции, а не в поле соседей.
    #[inline]
    const fn in_core(x: i32, y: i32, z: i32) -> bool {
        0 <= x && x < SECTION_SIZE && 0 <= y && y < SECTION_SIZE && 0 <= z && z < SECTION_SIZE
    }

    #[inline]
    fn index(x: i32, y: i32, z: i32) -> usize {
        debug_assert!(
            (-1..=SECTION_SIZE).contains(&x)
                && (-1..=SECTION_SIZE).contains(&y)
                && (-1..=SECTION_SIZE).contains(&z),
            "координата снимка вне -1..=16: ({x}, {y}, {z})"
        );
        let px = (x + 1) as usize;
        let py = (y + 1) as usize;
        let pz = (z + 1) as usize;
        // Порядок y, z, x — как в секции: проход по X идёт по подряд лежащей памяти.
        (py * PADDED_SIZE + pz) * PADDED_SIZE + px
    }

    /// Заполнить снимок из мира.
    ///
    /// Возвращает `false`, если колонки нет или секция вне колонки: мешить нечего.
    /// Соседние колонки читаются, если загружены; отсутствующий сосед считается
    /// воздухом, и грань на границе будет нарисована. Это намеренно: лучше лишняя
    /// грань, которая исчезнет при перемешивании после подгрузки соседа, чем дыра,
    /// сквозь которую видно пустоту.
    pub fn fill_from_world(
        &mut self,
        world: &VoxelWorld,
        position: ChunkPos,
        section_index: usize,
    ) -> bool {
        let Some(column) = world.column(position) else {
            return false;
        };
        if column.section(section_index).is_none() {
            return false;
        }

        self.position = position;
        self.section_index = section_index;
        self.base_y = Column::section_base_y(section_index);
        self.non_air = 0;

        let origin = position.origin(0);
        for y in -1..=SECTION_SIZE {
            for z in -1..=SECTION_SIZE {
                for x in -1..=SECTION_SIZE {
                    let world_pos = BlockPos::new(origin.x + x, self.base_y + y, origin.z + z);
                    let block = world.block(world_pos);
                    if Self::in_core(x, y, z) && !block.is_air() {
                        self.non_air += 1;
                    }
                    self.blocks[Self::index(x, y, z)] = block;
                }
            }
        }
        true
    }

    /// Заполнить снимок одной колонкой, без соседей по X и Z.
    ///
    /// Для тестов и бенчмарков: не требует собирать целый мир.
    pub fn fill_from_column(&mut self, column: &Column, section_index: usize) -> bool {
        if column.section(section_index).is_none() {
            return false;
        }
        self.position = column.position();
        self.section_index = section_index;
        self.base_y = Column::section_base_y(section_index);
        self.non_air = 0;

        for y in -1..=SECTION_SIZE {
            for z in -1..=SECTION_SIZE {
                for x in -1..=SECTION_SIZE {
                    let block = if (0..SECTION_SIZE).contains(&x) && (0..SECTION_SIZE).contains(&z)
                    {
                        column.get(x as u8, self.base_y + y, z as u8)
                    } else {
                        BlockId::AIR
                    };
                    if Self::in_core(x, y, z) && !block.is_air() {
                        self.non_air += 1;
                    }
                    self.blocks[Self::index(x, y, z)] = block;
                }
            }
        }
        true
    }

    /// Есть ли в самой секции хоть один непустой блок.
    ///
    /// Проверяется до мешинга: пустую секцию незачем просматривать по граням,
    /// а в колонке их обычно больше половины.
    #[inline]
    pub fn has_blocks(&self) -> bool {
        self.non_air > 0
    }

    /// Сколько непустых блоков в секции.
    #[inline]
    pub fn non_air(&self) -> u32 {
        self.non_air
    }
}

impl Default for PaddedSection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_world::ChunkPos;

    fn block(raw: u16) -> BlockId {
        BlockId::new(raw)
    }

    #[test]
    fn новый_снимок_пуст() {
        let view = PaddedSection::new();
        assert!(!view.has_blocks());
        for x in -1..=SECTION_SIZE {
            assert!(view.get(x, 0, 0).is_air());
        }
    }

    #[test]
    fn запись_и_чтение_включая_поля() {
        let mut view = PaddedSection::new();
        view.set(-1, -1, -1, block(1));
        view.set(16, 16, 16, block(2));
        view.set(0, 0, 0, block(3));
        assert_eq!(view.get(-1, -1, -1), block(1));
        assert_eq!(view.get(16, 16, 16), block(2));
        assert_eq!(view.get(0, 0, 0), block(3));
        // Поле не считается содержимым секции.
        let mut only_padding = PaddedSection::new();
        only_padding.set(-1, 0, 0, block(1));
        assert!(!only_padding.has_blocks());
    }

    #[test]
    fn индексы_не_пересекаются() {
        // Любая пара координат из -1..=16 должна давать разный индекс.
        let mut seen = vec![false; PADDED_VOLUME];
        for y in -1..=SECTION_SIZE {
            for z in -1..=SECTION_SIZE {
                for x in -1..=SECTION_SIZE {
                    let index = PaddedSection::index(x, y, z);
                    assert!(!seen[index], "повтор индекса на ({x}, {y}, {z})");
                    seen[index] = true;
                }
            }
        }
        assert!(seen.iter().all(|used| *used), "не все индексы использованы");
    }

    #[test]
    fn снимок_из_мира_берёт_соседей() {
        let mut world = VoxelWorld::new();
        let mut own = Column::new(ChunkPos::new(0, 0));
        own.set(0, 0, 0, block(5));
        world.insert(own);

        let mut neighbour = Column::new(ChunkPos::new(-1, 0));
        // Блок в соседней колонке вплотную к нашей границе: локальный x = 15.
        neighbour.set(15, 0, 0, block(7));
        world.insert(neighbour);

        let mut view = PaddedSection::new();
        // Секция с мировой Y = 0.
        let section_index = Column::section_index(0).expect("Y в мире");
        assert!(view.fill_from_world(&world, ChunkPos::new(0, 0), section_index));

        assert_eq!(view.get(0, 0, 0), block(5));
        assert_eq!(view.get(-1, 0, 0), block(7), "сосед должен попасть в поле");
        assert!(view.has_blocks());
    }

    #[test]
    fn незагруженный_сосед_читается_как_воздух() {
        let mut world = VoxelWorld::new();
        let mut own = Column::new(ChunkPos::new(0, 0));
        own.set(0, 0, 0, block(5));
        world.insert(own);

        let mut view = PaddedSection::new();
        let section_index = Column::section_index(0).expect("Y в мире");
        assert!(view.fill_from_world(&world, ChunkPos::new(0, 0), section_index));
        assert!(view.get(-1, 0, 0).is_air());
    }

    #[test]
    fn снимок_незагруженной_колонки_не_собирается() {
        let world = VoxelWorld::new();
        let mut view = PaddedSection::new();
        assert!(!view.fill_from_world(&world, ChunkPos::new(3, 3), 0));
    }

    #[test]
    fn снимок_из_колонки_без_соседей() {
        let mut column = Column::new(ChunkPos::new(2, 2));
        column.set(0, 0, 0, block(9));
        let mut view = PaddedSection::new();
        let section_index = Column::section_index(0).expect("Y в мире");
        assert!(view.fill_from_column(&column, section_index));
        assert_eq!(view.get(0, 0, 0), block(9));
        assert!(view.get(-1, 0, 0).is_air());
        assert_eq!(view.position(), ChunkPos::new(2, 2));
        assert_eq!(view.base_y(), 0);
    }
}
