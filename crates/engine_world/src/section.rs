//! Секция 16³: хранение вокселей через палитру и упакованные индексы.
//!
//! Три представления, переход между ними автоматический:
//!
//! | Представление | Когда | Память на секцию |
//! |---|---|---|
//! | `Uniform`     | вся секция из одного блока      | 2 байта |
//! | `Palette`     | до 256 разных блоков            | 0.5–4 КиБ + палитра |
//! | `Direct`      | больше 256 разных блоков        | 8 КиБ |
//!
//! Однородная секция — не редкий случай: над рельефом это воздух, под ним камень.
//! На радиусе 12 чанков таких секций большинство, поэтому отдельное представление
//! для них экономит не проценты, а разы.

use crate::{BlockId, LocalPos, SECTION_VOLUME};

/// Сколько разных блоков в секции до перехода на `Direct`.
const MAX_PALETTE_LEN: usize = 256;

/// Хранение вокселей секции.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Storage {
    /// Вся секция заполнена одним блоком.
    Uniform(BlockId),
    /// Палитра и упакованные индексы в неё.
    Palette {
        palette: Vec<BlockId>,
        /// Бит на воксель: 1, 2, 4 или 8. Делитель 64, поэтому индекс
        /// никогда не пересекает границу слова.
        bits: u8,
        data: Box<[u64]>,
    },
    /// Блок на воксель без палитры.
    Direct(Box<[BlockId; SECTION_VOLUME]>),
}

/// Секция 16³.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    storage: Storage,
    /// Сколько вокселей не являются воздухом. Нужен, чтобы пропускать пустые
    /// секции в мешинге, не просматривая 4096 вокселей.
    non_air: u16,
}

impl Default for Section {
    fn default() -> Self {
        Self::uniform(BlockId::AIR)
    }
}

impl Section {
    /// Секция из одного блока.
    pub fn uniform(block: BlockId) -> Self {
        Self {
            storage: Storage::Uniform(block),
            non_air: if block.is_air() {
                0
            } else {
                SECTION_VOLUME as u16
            },
        }
    }

    /// Блок в позиции.
    #[inline]
    pub fn get(&self, pos: LocalPos) -> BlockId {
        self.get_by_index(pos.index())
    }

    /// Блок по индексу внутри секции.
    ///
    /// # Паника
    ///
    /// Если индекс не меньше [`SECTION_VOLUME`].
    #[inline]
    pub fn get_by_index(&self, index: usize) -> BlockId {
        assert!(index < SECTION_VOLUME, "индекс вне секции");
        match &self.storage {
            Storage::Uniform(block) => *block,
            Storage::Palette {
                palette,
                bits,
                data,
            } => {
                let raw = read_packed(data, *bits, index);
                // Палитра не бывает короче числа использованных индексов,
                // но повреждённые данные не должны валить процесс.
                palette.get(raw).copied().unwrap_or(BlockId::AIR)
            }
            Storage::Direct(blocks) => blocks[index],
        }
    }

    /// Записать блок. Возвращает предыдущий.
    pub fn set(&mut self, pos: LocalPos, block: BlockId) -> BlockId {
        self.set_by_index(pos.index(), block)
    }

    /// Записать блок по индексу. Возвращает предыдущий.
    ///
    /// # Паника
    ///
    /// Если индекс не меньше [`SECTION_VOLUME`].
    pub fn set_by_index(&mut self, index: usize, block: BlockId) -> BlockId {
        assert!(index < SECTION_VOLUME, "индекс вне секции");

        let previous = self.get_by_index(index);
        if previous == block {
            return previous;
        }

        match &mut self.storage {
            Storage::Uniform(uniform) => {
                let uniform = *uniform;
                // Однородной секции больше нет: разворачиваем в палитру из двух
                // значений, то есть один бит на воксель.
                self.storage = Storage::from_uniform_with(uniform, block);
                self.write_unchecked(index, block);
            }
            Storage::Palette { .. } | Storage::Direct(_) => {
                self.write_unchecked(index, block);
            }
        }

        if previous.is_air() && !block.is_air() {
            self.non_air += 1;
        } else if !previous.is_air() && block.is_air() {
            self.non_air -= 1;
        }

        previous
    }

    /// Сколько вокселей не воздух.
    #[inline]
    pub fn non_air(&self) -> u16 {
        self.non_air
    }

    /// Секция целиком из воздуха.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.non_air == 0
    }

    /// Если секция состоит из одного блока — какого.
    #[inline]
    pub fn uniform_block(&self) -> Option<BlockId> {
        match &self.storage {
            Storage::Uniform(block) => Some(*block),
            _ => None,
        }
    }

    /// Сколько байт занимают данные секции (без учёта самой структуры).
    /// Для замеров памяти и тестов на представление.
    pub fn data_bytes(&self) -> usize {
        match &self.storage {
            Storage::Uniform(_) => 0,
            Storage::Palette { palette, data, .. } => {
                palette.len() * size_of::<BlockId>() + data.len() * size_of::<u64>()
            }
            Storage::Direct(_) => SECTION_VOLUME * size_of::<BlockId>(),
        }
    }

    /// Привести представление к минимальному: однородную секцию свернуть в
    /// `Uniform`, палитру сжать до нужного числа бит.
    ///
    /// Вызывается после массовой записи (генерация колонки), а не на каждый `set`:
    /// пересчёт стоит проход по секции.
    pub fn shrink(&mut self) {
        // Однородная секция уже минимальна. Без этой проверки сжатие колонки
        // проходило бы по 4096 вокселей в каждой из секций воздуха и камня,
        // а их в колонке большинство.
        if matches!(self.storage, Storage::Uniform(_)) {
            return;
        }

        let first = self.get_by_index(0);
        if (0..SECTION_VOLUME).all(|index| self.get_by_index(index) == first) {
            self.storage = Storage::Uniform(first);
            self.non_air = if first.is_air() {
                0
            } else {
                SECTION_VOLUME as u16
            };
            return;
        }

        // Собрать фактически используемые блоки и перепаковать под них.
        let mut used: Vec<BlockId> = Vec::new();
        for index in 0..SECTION_VOLUME {
            let block = self.get_by_index(index);
            if !used.contains(&block) {
                used.push(block);
            }
        }
        if used.len() > MAX_PALETTE_LEN {
            return;
        }
        used.sort_unstable();

        let bits = bits_for(used.len());
        let mut data = vec![0u64; packed_words(bits)].into_boxed_slice();
        for index in 0..SECTION_VOLUME {
            let block = self.get_by_index(index);
            let raw = used
                .binary_search(&block)
                .expect("блок только что был собран в used");
            write_packed(&mut data, bits, index, raw);
        }
        self.storage = Storage::Palette {
            palette: used,
            bits,
            data,
        };
    }

    /// Записать блок, расширив представление при необходимости.
    /// Счётчик `non_air` не трогает — это делает вызывающий.
    fn write_unchecked(&mut self, index: usize, block: BlockId) {
        loop {
            match &mut self.storage {
                Storage::Uniform(uniform) => {
                    let uniform = *uniform;
                    self.storage = Storage::from_uniform_with(uniform, block);
                }
                Storage::Palette {
                    palette,
                    bits,
                    data,
                } => {
                    if let Some(raw) = palette.iter().position(|entry| *entry == block) {
                        write_packed(data, *bits, index, raw);
                        return;
                    }
                    if palette.len() < capacity_for(*bits) {
                        palette.push(block);
                        let raw = palette.len() - 1;
                        write_packed(data, *bits, index, raw);
                        return;
                    }
                    // Палитра переполнена: шире биты или уход в Direct.
                    self.grow();
                }
                Storage::Direct(blocks) => {
                    blocks[index] = block;
                    return;
                }
            }
        }
    }

    /// Расширить представление на один шаг: 1 → 2 → 4 → 8 бит → `Direct`.
    fn grow(&mut self) {
        let Storage::Palette {
            palette,
            bits,
            data,
        } = &self.storage
        else {
            return;
        };

        if *bits >= 8 {
            let mut blocks = Box::new([BlockId::AIR; SECTION_VOLUME]);
            for (index, slot) in blocks.iter_mut().enumerate() {
                let raw = read_packed(data, *bits, index);
                *slot = palette.get(raw).copied().unwrap_or(BlockId::AIR);
            }
            self.storage = Storage::Direct(blocks);
            return;
        }

        let new_bits = bits * 2;
        let mut new_data = vec![0u64; packed_words(new_bits)].into_boxed_slice();
        for index in 0..SECTION_VOLUME {
            let raw = read_packed(data, *bits, index);
            write_packed(&mut new_data, new_bits, index, raw);
        }
        self.storage = Storage::Palette {
            palette: palette.clone(),
            bits: new_bits,
            data: new_data,
        };
    }
}

impl Storage {
    /// Развернуть однородную секцию в палитру, добавив второй блок.
    fn from_uniform_with(uniform: BlockId, extra: BlockId) -> Self {
        let bits = 1;
        Self::Palette {
            palette: vec![uniform, extra],
            bits,
            // Все воксели пока указывают на нулевой элемент палитры, то есть
            // на исходный однородный блок.
            data: vec![0u64; packed_words(bits)].into_boxed_slice(),
        }
    }
}

/// Сколько разных значений вмещает индекс заданной ширины.
#[inline]
const fn capacity_for(bits: u8) -> usize {
    1usize << bits
}

/// Минимальная ширина индекса для заданного числа элементов палитры.
#[inline]
const fn bits_for(len: usize) -> u8 {
    match len {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    }
}

/// Сколько 64-битных слов нужно для секции при заданной ширине индекса.
#[inline]
const fn packed_words(bits: u8) -> usize {
    SECTION_VOLUME * bits as usize / 64
}

/// Прочитать упакованный индекс.
#[inline]
fn read_packed(data: &[u64], bits: u8, index: usize) -> usize {
    let per_word = 64 / bits as usize;
    let word = index / per_word;
    let shift = (index % per_word) * bits as usize;
    let mask = (1u64 << bits) - 1;
    ((data[word] >> shift) & mask) as usize
}

/// Записать упакованный индекс.
#[inline]
fn write_packed(data: &mut [u64], bits: u8, index: usize, value: usize) {
    let per_word = 64 / bits as usize;
    let word = index / per_word;
    let shift = (index % per_word) * bits as usize;
    let mask = (1u64 << bits) - 1;
    data[word] = (data[word] & !(mask << shift)) | ((value as u64 & mask) << shift);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(raw: u16) -> BlockId {
        BlockId::new(raw)
    }

    #[test]
    fn новая_секция_из_воздуха() {
        let section = Section::default();
        assert!(section.is_empty());
        assert_eq!(section.non_air(), 0);
        assert_eq!(section.uniform_block(), Some(BlockId::AIR));
        assert_eq!(
            section.data_bytes(),
            0,
            "однородная секция не хранит данных"
        );
    }

    #[test]
    fn однородная_секция_не_занимает_память_на_воксели() {
        let section = Section::uniform(block(1));
        assert_eq!(section.non_air(), SECTION_VOLUME as u16);
        assert_eq!(section.data_bytes(), 0);
        for index in 0..SECTION_VOLUME {
            assert_eq!(section.get_by_index(index), block(1));
        }
    }

    #[test]
    fn запись_и_чтение() {
        let mut section = Section::default();
        let pos = LocalPos::new(3, 7, 11);
        assert_eq!(section.set(pos, block(5)), BlockId::AIR);
        assert_eq!(section.get(pos), block(5));
        assert_eq!(section.non_air(), 1);
        // Соседние воксели не затронуты.
        assert_eq!(section.get(LocalPos::new(4, 7, 11)), BlockId::AIR);
        assert_eq!(section.get(LocalPos::new(3, 8, 11)), BlockId::AIR);
    }

    #[test]
    fn счётчик_непустых_вокселей() {
        let mut section = Section::default();
        section.set(LocalPos::new(0, 0, 0), block(1));
        section.set(LocalPos::new(1, 0, 0), block(2));
        assert_eq!(section.non_air(), 2);

        // Перезапись непустого непустым не меняет счётчик.
        section.set(LocalPos::new(0, 0, 0), block(3));
        assert_eq!(section.non_air(), 2);

        // Возврат в воздух уменьшает.
        section.set(LocalPos::new(0, 0, 0), BlockId::AIR);
        assert_eq!(section.non_air(), 1);
        assert!(!section.is_empty());
        section.set(LocalPos::new(1, 0, 0), BlockId::AIR);
        assert!(section.is_empty());
    }

    #[test]
    fn все_воксели_различимы_при_полном_заполнении() {
        // Пишем всю секцию псевдослучайными блоками и читаем обратно:
        // так ловится любая ошибка в упаковке индексов.
        let mut section = Section::default();
        let expected: Vec<BlockId> = (0..SECTION_VOLUME)
            .map(|index| block(((index * 7 + 3) % 300) as u16))
            .collect();
        for (index, want) in expected.iter().enumerate() {
            section.set_by_index(index, *want);
        }
        for (index, want) in expected.iter().enumerate() {
            assert_eq!(section.get_by_index(index), *want, "воксель {index}");
        }
    }

    #[test]
    fn представление_расширяется_по_числу_блоков() {
        let mut section = Section::default();
        // Два блока — один бит на воксель: 4096 бит = 512 байт.
        section.set_by_index(0, block(1));
        assert_eq!(section.data_bytes(), 512 + 2 * size_of::<BlockId>());

        // Три блока — два бита: 1 КиБ.
        section.set_by_index(1, block(2));
        assert_eq!(section.data_bytes(), 1024 + 3 * size_of::<BlockId>());

        // Шесть блоков (воздух тоже занимает элемент палитры) — четыре бита: 2 КиБ.
        for (index, raw) in (3..6u16).enumerate() {
            section.set_by_index(index + 2, block(raw));
        }
        assert_eq!(section.data_bytes(), 2048 + 6 * size_of::<BlockId>());
    }

    #[test]
    fn больше_256_блоков_уходит_в_direct() {
        let mut section = Section::default();
        for raw in 0..300u16 {
            section.set_by_index(raw as usize, block(raw + 1));
        }
        assert_eq!(section.data_bytes(), SECTION_VOLUME * size_of::<BlockId>());
        // Данные целы после перехода.
        for raw in 0..300u16 {
            assert_eq!(section.get_by_index(raw as usize), block(raw + 1));
        }
    }

    #[test]
    fn данные_целы_при_каждом_расширении() {
        let mut section = Section::default();
        for raw in 1..=20u16 {
            section.set_by_index(raw as usize, block(raw));
            // После каждого расширения всё записанное ранее должно читаться.
            for earlier in 1..=raw {
                assert_eq!(
                    section.get_by_index(earlier as usize),
                    block(earlier),
                    "после добавления {raw} сломался воксель {earlier}"
                );
            }
        }
    }

    #[test]
    fn shrink_свёртывает_однородную_секцию() {
        let mut section = Section::default();
        for index in 0..SECTION_VOLUME {
            section.set_by_index(index, block(1));
        }
        assert!(section.uniform_block().is_none(), "пока палитра");
        section.shrink();
        assert_eq!(section.uniform_block(), Some(block(1)));
        assert_eq!(section.data_bytes(), 0);
        assert_eq!(section.non_air(), SECTION_VOLUME as u16);
    }

    #[test]
    fn shrink_сжимает_палитру_после_удаления_блоков() {
        let mut section = Section::default();
        // Пять блоков — четыре бита.
        for raw in 1..=5u16 {
            section.set_by_index(raw as usize, block(raw));
        }
        let wide = section.data_bytes();
        // Оставляем два блока: воздух и один тип.
        for raw in 2..=5u16 {
            section.set_by_index(raw as usize, BlockId::AIR);
        }
        section.shrink();
        assert!(
            section.data_bytes() < wide,
            "после shrink {} должно быть меньше {wide}",
            section.data_bytes()
        );
        assert_eq!(section.get_by_index(1), block(1));
        assert_eq!(section.non_air(), 1);
    }

    #[test]
    fn shrink_сохраняет_данные() {
        let mut section = Section::default();
        let expected: Vec<BlockId> = (0..SECTION_VOLUME)
            .map(|index| block((index % 17) as u16))
            .collect();
        for (index, want) in expected.iter().enumerate() {
            section.set_by_index(index, *want);
        }
        section.shrink();
        for (index, want) in expected.iter().enumerate() {
            assert_eq!(section.get_by_index(index), *want, "воксель {index}");
        }
    }

    #[test]
    #[should_panic(expected = "индекс вне секции")]
    fn индекс_вне_секции_паникует() {
        let section = Section::default();
        let _ = section.get_by_index(SECTION_VOLUME);
    }

    #[test]
    fn упаковка_и_распаковка_индексов() {
        for bits in [1u8, 2, 4, 8] {
            let mut data = vec![0u64; packed_words(bits)].into_boxed_slice();
            let limit = capacity_for(bits);
            for index in 0..SECTION_VOLUME {
                write_packed(&mut data, bits, index, index % limit);
            }
            for index in 0..SECTION_VOLUME {
                assert_eq!(
                    read_packed(&data, bits, index),
                    index % limit,
                    "bits={bits} index={index}"
                );
            }
        }
    }
}
