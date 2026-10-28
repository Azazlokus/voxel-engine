//! Шесть граней вокселя.

/// Грань вокселя, она же направление к соседу.
///
/// Порядок вариантов задаёт нумерацию в упакованной вершине (3 бита) и порядок
/// слоёв в texture array, поэтому менять его нельзя без пересборки и того, и другого.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// В сторону +X (восток).
    PosX,
    /// В сторону -X (запад).
    NegX,
    /// Вверх, +Y.
    PosY,
    /// Вниз, -Y.
    NegY,
    /// В сторону +Z (юг).
    PosZ,
    /// В сторону -Z (север).
    NegZ,
}

impl Face {
    /// Все грани в порядке нумерации.
    pub const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    /// Номер грани, `0..6`. Он же значение в упакованной вершине.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Смещение к соседнему вокселю через эту грань.
    #[inline]
    pub const fn offset(self) -> (i32, i32, i32) {
        match self {
            Face::PosX => (1, 0, 0),
            Face::NegX => (-1, 0, 0),
            Face::PosY => (0, 1, 0),
            Face::NegY => (0, -1, 0),
            Face::PosZ => (0, 0, 1),
            Face::NegZ => (0, 0, -1),
        }
    }

    /// Нормаль грани.
    #[inline]
    pub const fn normal(self) -> [f32; 3] {
        let (x, y, z) = self.offset();
        [x as f32, y as f32, z as f32]
    }

    /// Противоположная грань.
    #[inline]
    pub const fn opposite(self) -> Face {
        match self {
            Face::PosX => Face::NegX,
            Face::NegX => Face::PosX,
            Face::PosY => Face::NegY,
            Face::NegY => Face::PosY,
            Face::PosZ => Face::NegZ,
            Face::NegZ => Face::PosZ,
        }
    }

    /// Это верхняя грань?
    #[inline]
    pub const fn is_top(self) -> bool {
        matches!(self, Face::PosY)
    }

    /// Это нижняя грань?
    #[inline]
    pub const fn is_bottom(self) -> bool {
        matches!(self, Face::NegY)
    }

    /// Это боковая грань?
    #[inline]
    pub const fn is_side(self) -> bool {
        !self.is_top() && !self.is_bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn номера_граней_идут_подряд() {
        for (expected, face) in Face::ALL.iter().enumerate() {
            assert_eq!(face.index(), expected);
        }
        assert_eq!(Face::ALL.len(), 6);
    }

    #[test]
    fn номер_грани_влезает_в_три_бита() {
        for face in Face::ALL {
            assert!(face.index() < 8, "{face:?} не влезает в 3 бита");
        }
    }

    #[test]
    fn противоположная_грань_взаимна() {
        for face in Face::ALL {
            assert_eq!(face.opposite().opposite(), face);
            assert_ne!(face.opposite(), face);
        }
    }

    #[test]
    fn смещение_противоположной_грани_обратно() {
        for face in Face::ALL {
            let (x, y, z) = face.offset();
            let (ox, oy, oz) = face.opposite().offset();
            assert_eq!((x + ox, y + oy, z + oz), (0, 0, 0), "{face:?}");
        }
    }

    #[test]
    fn смещение_единичное() {
        for face in Face::ALL {
            let (x, y, z) = face.offset();
            assert_eq!(x.abs() + y.abs() + z.abs(), 1, "{face:?}");
        }
    }

    #[test]
    fn верх_низ_и_бока_не_пересекаются() {
        let top: Vec<Face> = Face::ALL.into_iter().filter(|f| f.is_top()).collect();
        let bottom: Vec<Face> = Face::ALL.into_iter().filter(|f| f.is_bottom()).collect();
        let sides = Face::ALL.into_iter().filter(|f| f.is_side()).count();
        assert_eq!(top, vec![Face::PosY]);
        assert_eq!(bottom, vec![Face::NegY]);
        assert_eq!(sides, 4);
    }

    #[test]
    fn нормаль_совпадает_со_смещением() {
        for face in Face::ALL {
            let (x, y, z) = face.offset();
            assert_eq!(face.normal(), [x as f32, y as f32, z as f32]);
        }
    }
}
