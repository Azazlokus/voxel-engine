//! Блоки: числовой ID, свойства, нужные миру и мешеру, и их реестр.

use bevy_ecs::resource::Resource;
use engine_registry::{Frozen, Identifier, RawId, Registry, RegistryError};
use thiserror::Error;

/// Числовой ID блока.
///
/// Шириной `u16` — палитра секции хранит эти значения, и 16 бит на блок
/// это предел, при котором секция без палитры (`Direct`) занимает 8 КиБ.
/// Значение осмысленно только внутри текущего запуска: см. docs/adr/0001.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(u16);

impl BlockId {
    /// Воздух. Всегда ноль — см. docs/adr/0002.
    pub const AIR: Self = Self(0);

    /// Сколько блоков вмещает числовой ID.
    pub const MAX_BLOCKS: usize = u16::MAX as usize + 1;

    /// Создать из числа.
    #[inline]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// Числовое значение.
    #[inline]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Значение как индекс массива.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Это воздух?
    #[inline]
    pub const fn is_air(self) -> bool {
        self.0 == Self::AIR.0
    }
}

/// Свойства блока, нужные миру и мешеру.
///
/// Это не полное описание блока: текстуры граней, твёрдость и прочее читает
/// из RON `engine_assets` и отдаёт рендеру. Здесь только то, без чего нельзя
/// построить меш или посчитать заполненность секции.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockProperties {
    /// Полностью перекрывает обзор: грань соседа, обращённая к такому блоку,
    /// не рисуется.
    pub opaque: bool,
    /// Собственная светимость, `0..=15`. Пока не используется рендером.
    pub light: u8,
    /// Блок проходим и не имеет геометрии (воздух и подобные).
    pub empty: bool,
}

impl BlockProperties {
    /// Свойства воздуха.
    pub const AIR: Self = Self {
        opaque: false,
        light: 0,
        empty: true,
    };

    /// Обычный непрозрачный блок.
    pub const SOLID: Self = Self {
        opaque: true,
        light: 0,
        empty: false,
    };
}

/// Ошибки реестра блоков.
#[derive(Debug, Error)]
pub enum BlockRegistryError {
    /// Ошибка нижележащего реестра.
    #[error(transparent)]
    Registry(#[from] RegistryError),

    /// Не зарегистрирован воздух.
    #[error("блок {0} обязателен: мир заполняется воздухом по умолчанию")]
    MissingAir(Identifier),

    /// Блоков больше, чем вмещает [`BlockId`].
    #[error("блоков {count}, предел {max}: не вмещается в BlockId")]
    TooManyBlocks {
        /// Сколько блоков зарегистрировано.
        count: usize,
        /// Предел.
        max: usize,
    },
}

/// Текстовый ID воздуха.
pub const AIR_ID: &str = "base:air";

/// Реестр блоков на этапе наполнения.
///
/// Живёт ресурсом ECS между установкой плагинов и заморозкой: плагины контента
/// дописывают в него свои блоки системами этапа `Startup`.
#[derive(Debug, Resource)]
pub struct BlockRegistry {
    inner: Registry<BlockProperties>,
}

impl Default for BlockRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockRegistry {
    /// Пустой реестр.
    pub fn new() -> Self {
        Self {
            inner: Registry::new("blocks"),
        }
    }

    /// Зарегистрировать блок.
    pub fn register(
        &mut self,
        id: Identifier,
        properties: BlockProperties,
    ) -> Result<(), BlockRegistryError> {
        self.inner.register(id, properties)?;
        Ok(())
    }

    /// Сколько блоков зарегистрировано.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Пуст ли реестр.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Выдать числовые ID.
    ///
    /// Воздух получает ноль, остальные блоки идут по возрастанию `Identifier`
    /// (см. docs/adr/0001 и docs/adr/0002).
    pub fn freeze(self) -> Result<Blocks, BlockRegistryError> {
        if self.inner.len() > BlockId::MAX_BLOCKS {
            return Err(BlockRegistryError::TooManyBlocks {
                count: self.inner.len(),
                max: BlockId::MAX_BLOCKS,
            });
        }

        let air = Identifier::parse(AIR_ID).expect("AIR_ID — корректный идентификатор");
        let frozen = self.inner.freeze()?;
        let air_raw = frozen
            .raw_id(&air)
            .ok_or_else(|| BlockRegistryError::MissingAir(air.clone()))?;

        // Воздух переносится в ноль, а освободившееся место занимает блок,
        // который стоял нулевым. Остальные ID остаются на своих местах.
        let mut properties: Vec<BlockProperties> = frozen.values().to_vec();
        let mut ids: Vec<Identifier> = frozen
            .iter()
            .map(|(_, id, _)| id.clone())
            .collect::<Vec<_>>();
        properties.swap(0, air_raw.index());
        ids.swap(0, air_raw.index());

        Ok(Blocks {
            properties: properties.into_boxed_slice(),
            ids: ids.into_boxed_slice(),
            frozen,
            air_raw,
        })
    }
}

/// Замороженный реестр блоков.
#[derive(Debug)]
pub struct Blocks {
    properties: Box<[BlockProperties]>,
    ids: Box<[Identifier]>,
    frozen: Frozen<BlockProperties>,
    /// Какой `RawId` был у воздуха до переноса в ноль.
    air_raw: RawId,
}

impl Blocks {
    /// Сколько блоков.
    #[inline]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Пуст ли реестр.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Свойства блока. Для неизвестного ID — свойства воздуха: мешер не должен
    /// падать на повреждённых данных.
    #[inline]
    pub fn properties(&self, block: BlockId) -> BlockProperties {
        self.properties
            .get(block.index())
            .copied()
            .unwrap_or(BlockProperties::AIR)
    }

    /// Непрозрачен ли блок.
    #[inline]
    pub fn is_opaque(&self, block: BlockId) -> bool {
        self.properties(block).opaque
    }

    /// Пуст ли блок (воздух и подобные).
    #[inline]
    pub fn is_empty_block(&self, block: BlockId) -> bool {
        self.properties(block).empty
    }

    /// Числовой ID по текстовому.
    pub fn id(&self, id: &Identifier) -> Option<BlockId> {
        let raw = self.frozen.raw_id(id)?;
        Some(self.remap(raw))
    }

    /// Текстовый ID по числовому.
    pub fn identifier(&self, block: BlockId) -> Option<&Identifier> {
        self.ids.get(block.index())
    }

    /// Все блоки по возрастанию числового ID.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (BlockId, &Identifier, BlockProperties)> {
        self.ids
            .iter()
            .zip(self.properties.iter())
            .enumerate()
            .map(|(raw, (id, properties))| (BlockId::new(raw as u16), id, *properties))
    }

    /// Перевести `RawId` нижележащего реестра в [`BlockId`] с учётом переноса воздуха.
    #[inline]
    fn remap(&self, raw: RawId) -> BlockId {
        let index = if raw == self.air_raw {
            0
        } else if raw.index() == 0 {
            self.air_raw.index()
        } else {
            raw.index()
        };
        BlockId::new(index as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("корректный id")
    }

    fn реестр(blocks: &[&str]) -> Blocks {
        let mut registry = BlockRegistry::new();
        registry
            .register(id(AIR_ID), BlockProperties::AIR)
            .expect("воздух регистрируется");
        for name in blocks {
            registry
                .register(id(name), BlockProperties::SOLID)
                .expect("блок регистрируется");
        }
        registry.freeze().expect("заморозка удалась")
    }

    #[test]
    fn воздух_получает_нулевой_id() {
        let blocks = реестр(&["base:dirt", "base:stone"]);
        assert_eq!(blocks.id(&id(AIR_ID)), Some(BlockId::AIR));
        assert!(BlockId::AIR.is_air());
        assert_eq!(blocks.identifier(BlockId::AIR), Some(&id(AIR_ID)));
    }

    #[test]
    fn воздух_нулевой_даже_если_сортировка_ставит_его_не_первым() {
        // "aaa:early" сортируется раньше "base:air", и без переноса воздух
        // получил бы ненулевой ID.
        let blocks = реестр(&["aaa:early", "base:stone"]);
        assert_eq!(blocks.id(&id(AIR_ID)), Some(BlockId::AIR));
        assert_eq!(blocks.identifier(BlockId::AIR), Some(&id(AIR_ID)));

        // Блок, стоявший нулевым, никуда не потерялся.
        let early = blocks.id(&id("aaa:early")).expect("блок есть");
        assert_ne!(early, BlockId::AIR);
        assert_eq!(blocks.identifier(early), Some(&id("aaa:early")));
    }

    #[test]
    fn числовой_и_текстовый_id_согласованы() {
        let blocks = реестр(&["aaa:early", "base:dirt", "base:stone", "zzz:late"]);
        for (block, text, _) in blocks.iter() {
            assert_eq!(blocks.id(text), Some(block), "{text} -> {block:?}");
            assert_eq!(blocks.identifier(block), Some(text));
        }
    }

    #[test]
    fn свойства_доступны_по_id() {
        let blocks = реестр(&["base:stone"]);
        let stone = blocks.id(&id("base:stone")).expect("блок есть");
        assert!(blocks.is_opaque(stone));
        assert!(!blocks.is_empty_block(stone));
        assert!(!blocks.is_opaque(BlockId::AIR));
        assert!(blocks.is_empty_block(BlockId::AIR));
    }

    #[test]
    fn неизвестный_id_даёт_свойства_воздуха() {
        let blocks = реестр(&["base:stone"]);
        let unknown = BlockId::new(9999);
        assert_eq!(blocks.properties(unknown), BlockProperties::AIR);
    }

    #[test]
    fn без_воздуха_заморозка_не_удаётся() {
        let mut registry = BlockRegistry::new();
        registry
            .register(id("base:stone"), BlockProperties::SOLID)
            .expect("блок регистрируется");
        assert!(matches!(
            registry.freeze(),
            Err(BlockRegistryError::MissingAir(_))
        ));
    }

    #[test]
    fn дубликат_блока_отвергается() {
        let mut registry = BlockRegistry::new();
        registry
            .register(id("base:stone"), BlockProperties::SOLID)
            .expect("блок регистрируется");
        assert!(matches!(
            registry.register(id("base:stone"), BlockProperties::SOLID),
            Err(BlockRegistryError::Registry(
                RegistryError::Duplicate { .. }
            ))
        ));
    }
}
