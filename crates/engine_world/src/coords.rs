//! Координаты: мировые, чанковые, локальные — и переводы между ними.
//!
//! Все переводы сделаны битовыми операциями, а не делением. `x / 16` и `x % 16`
//! в Rust округляют к нулю, поэтому для отрицательных координат дают неверный чанк
//! и отрицательный остаток: блоки `x = -1` и `x = 0` попали бы в один чанк, а на
//! осях появились бы швы и дыры. `x >> 4` округляет вниз всегда, `x & 15` всегда
//! неотрицателен.

/// Сторона секции в блоках.
pub const SECTION_SIZE: i32 = 16;
/// Сколько блоков в секции.
pub const SECTION_VOLUME: usize = 16 * 16 * 16;
/// Сколько бит занимает локальная координата.
const LOCAL_BITS: u32 = 4;
/// Маска локальной координаты.
const LOCAL_MASK: i32 = SECTION_SIZE - 1;

/// Позиция блока в мире.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BlockPos {
    /// Ось X.
    pub x: i32,
    /// Ось Y, вверх.
    pub y: i32,
    /// Ось Z.
    pub z: i32,
}

impl BlockPos {
    /// Новая позиция.
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Колонка чанков, которой принадлежит блок.
    #[inline]
    pub const fn chunk(self) -> ChunkPos {
        ChunkPos::new(self.x >> LOCAL_BITS, self.z >> LOCAL_BITS)
    }

    /// Индекс секции по вертикали (не её позиция в массиве колонки).
    #[inline]
    pub const fn section_y(self) -> i32 {
        self.y >> LOCAL_BITS
    }

    /// Локальная позиция внутри секции.
    #[inline]
    pub const fn local(self) -> LocalPos {
        LocalPos::new(
            (self.x & LOCAL_MASK) as u8,
            (self.y & LOCAL_MASK) as u8,
            (self.z & LOCAL_MASK) as u8,
        )
    }

    /// Сдвинуть позицию.
    #[inline]
    pub const fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }
}

/// Позиция колонки чанков в мире. Вертикали нет: колонка целиком.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ChunkPos {
    /// Ось X в чанках.
    pub x: i32,
    /// Ось Z в чанках.
    pub z: i32,
}

impl ChunkPos {
    /// Новая позиция колонки.
    #[inline]
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    /// Мировая позиция блока с локальными координатами `(0, y, 0)`.
    #[inline]
    pub const fn origin(self, y: i32) -> BlockPos {
        BlockPos::new(self.x << LOCAL_BITS, y, self.z << LOCAL_BITS)
    }

    /// Сдвинуть на заданное число чанков.
    #[inline]
    pub const fn offset(self, dx: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.z + dz)
    }

    /// Квадрат расстояния до другой колонки в чанках.
    ///
    /// Квадрат, а не расстояние — чтобы сортировать по дальности без корня.
    #[inline]
    pub const fn distance_squared(self, other: Self) -> i64 {
        let dx = (self.x - other.x) as i64;
        let dz = (self.z - other.z) as i64;
        dx * dx + dz * dz
    }
}

/// Локальная позиция внутри секции 16³. Каждая компонента в `0..16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LocalPos {
    x: u8,
    y: u8,
    z: u8,
}

impl LocalPos {
    /// Новая локальная позиция.
    ///
    /// # Паника
    ///
    /// Если любая компонента не меньше 16.
    #[inline]
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        assert!(
            x < SECTION_SIZE as u8 && y < SECTION_SIZE as u8 && z < SECTION_SIZE as u8,
            "локальная координата должна быть в 0..16"
        );
        Self { x, y, z }
    }

    /// Локальная позиция из индекса в секции.
    ///
    /// # Паника
    ///
    /// Если индекс не меньше [`SECTION_VOLUME`].
    #[inline]
    pub const fn from_index(index: usize) -> Self {
        assert!(index < SECTION_VOLUME, "индекс вне секции");
        Self {
            x: (index & 15) as u8,
            z: ((index >> 4) & 15) as u8,
            y: ((index >> 8) & 15) as u8,
        }
    }

    /// Ось X.
    #[inline]
    pub const fn x(self) -> u8 {
        self.x
    }

    /// Ось Y.
    #[inline]
    pub const fn y(self) -> u8 {
        self.y
    }

    /// Ось Z.
    #[inline]
    pub const fn z(self) -> u8 {
        self.z
    }

    /// Индекс в секции.
    ///
    /// Порядок `y, z, x` — X меняется быстрее всего, поэтому проход по X идёт
    /// по подряд лежащей памяти. Это важно и для мешинга, и для генерации.
    #[inline]
    pub const fn index(self) -> usize {
        (self.y as usize) << 8 | (self.z as usize) << 4 | (self.x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn положительные_координаты() {
        let pos = BlockPos::new(0, 0, 0);
        assert_eq!(pos.chunk(), ChunkPos::new(0, 0));
        assert_eq!(pos.local(), LocalPos::new(0, 0, 0));

        let pos = BlockPos::new(15, 15, 15);
        assert_eq!(pos.chunk(), ChunkPos::new(0, 0));
        assert_eq!(pos.local(), LocalPos::new(15, 15, 15));

        let pos = BlockPos::new(16, 16, 16);
        assert_eq!(pos.chunk(), ChunkPos::new(1, 1));
        assert_eq!(pos.section_y(), 1);
        assert_eq!(pos.local(), LocalPos::new(0, 0, 0));
    }

    #[test]
    fn отрицательные_координаты_не_склеиваются_с_нулём() {
        // Главный баг, из-за которого появляются швы на осях: деление к нулю
        // отправило бы -1 в чанк 0 вместе с блоком 0.
        let minus = BlockPos::new(-1, -1, -1);
        assert_eq!(minus.chunk(), ChunkPos::new(-1, -1));
        assert_eq!(minus.section_y(), -1);
        assert_eq!(minus.local(), LocalPos::new(15, 15, 15));

        let zero = BlockPos::new(0, 0, 0);
        assert_ne!(minus.chunk(), zero.chunk());
    }

    #[test]
    fn границы_чанков_на_отрицательной_оси() {
        for (world, chunk, local) in [
            (-17, -2, 15),
            (-16, -1, 0),
            (-15, -1, 1),
            (-1, -1, 15),
            (0, 0, 0),
            (15, 0, 15),
            (16, 1, 0),
        ] {
            let pos = BlockPos::new(world, world, world);
            assert_eq!(
                pos.chunk(),
                ChunkPos::new(chunk, chunk),
                "мировая {world} должна попасть в чанк {chunk}"
            );
            assert_eq!(pos.section_y(), chunk, "секция по Y для {world}");
            assert_eq!(
                pos.local(),
                LocalPos::new(local, local, local),
                "локальная для {world}"
            );
        }
    }

    #[test]
    fn локальная_координата_всегда_неотрицательна() {
        for world in -64..64 {
            let local = BlockPos::new(world, world, world).local();
            assert!(local.x() < 16 && local.y() < 16 && local.z() < 16);
        }
    }

    #[test]
    fn перевод_в_начало_чанка_и_обратно() {
        for cx in [-3i32, -1, 0, 5] {
            for cz in [-7i32, -1, 0, 2] {
                let chunk = ChunkPos::new(cx, cz);
                let origin = chunk.origin(0);
                assert_eq!(origin.chunk(), chunk);
                assert_eq!(origin.local(), LocalPos::new(0, 0, 0));
                // Последний блок чанка принадлежит тому же чанку.
                let last = origin.offset(15, 0, 15);
                assert_eq!(last.chunk(), chunk);
            }
        }
    }

    #[test]
    fn индекс_и_локальная_позиция_обратимы() {
        for index in 0..SECTION_VOLUME {
            assert_eq!(LocalPos::from_index(index).index(), index);
        }
    }

    #[test]
    fn x_меняется_быстрее_всего() {
        // Проход по X должен идти по подряд лежащей памяти.
        assert_eq!(
            LocalPos::new(0, 0, 0).index() + 1,
            LocalPos::new(1, 0, 0).index()
        );
        assert_eq!(
            LocalPos::new(0, 0, 0).index() + 16,
            LocalPos::new(0, 0, 1).index()
        );
        assert_eq!(
            LocalPos::new(0, 0, 0).index() + 256,
            LocalPos::new(0, 1, 0).index()
        );
    }

    #[test]
    fn квадрат_расстояния_между_колонками() {
        let a = ChunkPos::new(0, 0);
        assert_eq!(a.distance_squared(ChunkPos::new(3, 4)), 25);
        assert_eq!(a.distance_squared(ChunkPos::new(-3, -4)), 25);
        assert_eq!(a.distance_squared(a), 0);
    }

    #[test]
    #[should_panic(expected = "0..16")]
    fn локальная_координата_вне_диапазона_паникует() {
        let _ = LocalPos::new(16, 0, 0);
    }
}
