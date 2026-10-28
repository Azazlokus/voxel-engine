//! Числовой идентификатор записи в замороженном реестре.

use core::fmt;

/// Индекс записи в замороженном реестре.
///
/// Не хранится в файлах мира и не передаётся между запусками: значение зависит
/// от состава реестра, то есть от набора плагинов. Для сохранений используется
/// [`crate::Identifier`] (см. docs/adr/0001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawId(u32);

impl RawId {
    /// Предельное число записей в одном реестре.
    pub const MAX_ENTRIES: usize = u32::MAX as usize;

    /// Создать из индекса.
    #[inline]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Числовое значение.
    #[inline]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Значение как индекс массива.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for RawId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}
