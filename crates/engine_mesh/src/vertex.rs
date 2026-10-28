//! Упакованная вершина меша чанка.

use engine_world::Face;

/// Вершина меша: ровно 8 байт.
///
/// Раскладка:
///
/// ```text
/// lo: x:6  y:6  z:6  ao:2  normal:3  резерв:9
/// hi: tex:12  u:6  v:6  резерв:8
/// ```
///
/// Шесть бит на координату, а не пять: координата доходит до 32, а не до 16,
/// поэтому позже можно склеивать квады через границу секции, не меняя формат
/// и не пересобирая шейдер.
///
/// Резервные биты оставлены под освещение (`lo`) и под вариант блока (`hi`):
/// добавить их потом без смены размера вершины дешевле, чем расширять формат,
/// когда на него уже завязаны буферы GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(C)]
pub struct PackedVertex {
    /// Позиция, ambient occlusion и нормаль.
    pub lo: u32,
    /// Текстура и координаты внутри квада.
    pub hi: u32,
}

/// Сколько бит отведено координате позиции.
const POS_BITS: u32 = 6;
/// Предельная координата вершины.
pub const MAX_POS: u32 = (1 << POS_BITS) - 1;
/// Предельный индекс текстуры.
pub const MAX_TEXTURE: u32 = (1 << 12) - 1;
/// Предельный размер квада по стороне.
pub const MAX_QUAD_SIZE: u32 = (1 << 6) - 1;
/// Максимальный уровень ambient occlusion (самый светлый).
pub const AO_MAX: u8 = 3;

impl PackedVertex {
    /// Собрать вершину.
    ///
    /// Значения, не влезающие в свои поля, обрезаются по маске: проверять их
    /// в самом горячем цикле мешера дорого, а источник значений — сам мешер,
    /// и на корректность диапазонов есть тесты.
    #[inline]
    pub const fn new(
        position: (u32, u32, u32),
        ao: u8,
        face: Face,
        texture: u32,
        uv: (u32, u32),
    ) -> Self {
        let (x, y, z) = position;
        let (u, v) = uv;
        let lo = (x & MAX_POS)
            | ((y & MAX_POS) << 6)
            | ((z & MAX_POS) << 12)
            | (((ao as u32) & 0x3) << 18)
            | (((face as u32) & 0x7) << 20);
        let hi =
            (texture & MAX_TEXTURE) | ((u & MAX_QUAD_SIZE) << 12) | ((v & MAX_QUAD_SIZE) << 18);
        Self { lo, hi }
    }

    /// Позиция вершины внутри секции.
    #[inline]
    pub const fn position(self) -> (u32, u32, u32) {
        (
            self.lo & MAX_POS,
            (self.lo >> 6) & MAX_POS,
            (self.lo >> 12) & MAX_POS,
        )
    }

    /// Уровень ambient occlusion, `0..=3`. Ноль — самый тёмный угол.
    #[inline]
    pub const fn ao(self) -> u8 {
        ((self.lo >> 18) & 0x3) as u8
    }

    /// Номер грани.
    #[inline]
    pub const fn face_index(self) -> u32 {
        (self.lo >> 20) & 0x7
    }

    /// Индекс текстуры в texture array.
    #[inline]
    pub const fn texture(self) -> u32 {
        self.hi & MAX_TEXTURE
    }

    /// Координаты внутри квада в блоках: по ним шейдер повторяет текстуру.
    #[inline]
    pub const fn uv(self) -> (u32, u32) {
        (
            (self.hi >> 12) & MAX_QUAD_SIZE,
            (self.hi >> 18) & MAX_QUAD_SIZE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn вершина_занимает_восемь_байт() {
        assert_eq!(size_of::<PackedVertex>(), 8);
        assert_eq!(align_of::<PackedVertex>(), 4);
    }

    #[test]
    fn поля_распаковываются_обратно() {
        let vertex = PackedVertex::new((5, 12, 31), 2, Face::NegZ, 1234, (7, 9));
        assert_eq!(vertex.position(), (5, 12, 31));
        assert_eq!(vertex.ao(), 2);
        assert_eq!(vertex.face_index(), Face::NegZ as u32);
        assert_eq!(vertex.texture(), 1234);
        assert_eq!(vertex.uv(), (7, 9));
    }

    #[test]
    fn поля_не_перетекают_друг_в_друга() {
        // Максимумы во всех полях сразу: если маски пересекаются, значения
        // испортят соседей.
        let vertex = PackedVertex::new(
            (MAX_POS, MAX_POS, MAX_POS),
            AO_MAX,
            Face::NegZ,
            MAX_TEXTURE,
            (MAX_QUAD_SIZE, MAX_QUAD_SIZE),
        );
        assert_eq!(vertex.position(), (MAX_POS, MAX_POS, MAX_POS));
        assert_eq!(vertex.ao(), AO_MAX);
        assert_eq!(vertex.face_index(), Face::NegZ as u32);
        assert_eq!(vertex.texture(), MAX_TEXTURE);
        assert_eq!(vertex.uv(), (MAX_QUAD_SIZE, MAX_QUAD_SIZE));
    }

    #[test]
    fn все_грани_кодируются_различимо() {
        let mut seen = Vec::new();
        for face in Face::ALL {
            let vertex = PackedVertex::new((0, 0, 0), 0, face, 0, (0, 0));
            assert_eq!(vertex.face_index(), face.index() as u32);
            assert!(!seen.contains(&vertex.face_index()), "{face:?} повторяется");
            seen.push(vertex.face_index());
        }
    }

    #[test]
    fn координата_вмещает_границу_секции() {
        // 16 — это дальняя грань секции, и она обязана кодироваться.
        let vertex = PackedVertex::new((16, 16, 16), 0, Face::PosY, 0, (16, 16));
        assert_eq!(vertex.position(), (16, 16, 16));
        assert_eq!(vertex.uv(), (16, 16));
    }

    #[test]
    fn резервные_биты_остаются_нулевыми() {
        let vertex = PackedVertex::new(
            (MAX_POS, MAX_POS, MAX_POS),
            AO_MAX,
            Face::NegZ,
            MAX_TEXTURE,
            (MAX_QUAD_SIZE, MAX_QUAD_SIZE),
        );
        assert_eq!(vertex.lo >> 23, 0, "старшие 9 бит lo зарезервированы");
        assert_eq!(vertex.hi >> 24, 0, "старшие 8 бит hi зарезервированы");
    }
}
