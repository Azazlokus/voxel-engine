//! Реестр контента: регистрация по текстовому ID, заморозка в числовые.

use crate::{Identifier, RawId};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use thiserror::Error;
use tracing::debug;

/// Ошибки регистрации.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Такой идентификатор уже зарегистрирован.
    #[error("{id} уже зарегистрирован в реестре {registry}")]
    Duplicate {
        /// Повторно зарегистрированный ID.
        id: Identifier,
        /// Имя реестра.
        registry: &'static str,
    },

    /// Записей больше, чем вмещает числовой ID.
    #[error("в реестре {registry} {count} записей, предел {max}")]
    TooManyEntries {
        /// Имя реестра.
        registry: &'static str,
        /// Сколько записей.
        count: usize,
        /// Предел.
        max: usize,
    },
}

/// Реестр на этапе наполнения: плагины регистрируют контент по текстовым ID.
///
/// Числовых ID здесь ещё нет — они выдаются [`Registry::freeze`] после того,
/// как отработали все плагины.
#[derive(Debug)]
pub struct Registry<T> {
    name: &'static str,
    entries: HashMap<Identifier, T>,
}

impl<T> Registry<T> {
    /// Пустой реестр с именем для сообщений об ошибках и логов.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            entries: HashMap::new(),
        }
    }

    /// Имя реестра.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Зарегистрировать значение. Повторная регистрация того же ID — ошибка:
    /// молчаливая перезапись маскировала бы конфликт двух плагинов.
    pub fn register(&mut self, id: Identifier, value: T) -> Result<(), RegistryError> {
        match self.entries.entry(id) {
            Entry::Occupied(occupied) => Err(RegistryError::Duplicate {
                id: occupied.key().clone(),
                registry: self.name,
            }),
            Entry::Vacant(vacant) => {
                vacant.insert(value);
                Ok(())
            }
        }
    }

    /// Сколько записей зарегистрировано.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Пуст ли реестр.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Есть ли такой идентификатор.
    pub fn contains(&self, id: &Identifier) -> bool {
        self.entries.contains_key(id)
    }

    /// Выдать числовые ID и запретить дальнейшую регистрацию.
    ///
    /// ID назначаются по возрастанию [`Identifier`], а не по порядку регистрации:
    /// тогда числа не зависят от порядка установки плагинов вообще. Цена — добавление
    /// записи сдвигает ID соседей, поэтому сохранения мира обязаны хранить таблицу
    /// соответствия текстовых и числовых ID (см. docs/adr/0001).
    pub fn freeze(self) -> Result<Frozen<T>, RegistryError> {
        if self.entries.len() > RawId::MAX_ENTRIES {
            return Err(RegistryError::TooManyEntries {
                registry: self.name,
                count: self.entries.len(),
                max: RawId::MAX_ENTRIES,
            });
        }

        let mut entries: Vec<(Identifier, T)> = self.entries.into_iter().collect();
        entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

        let mut ids = Vec::with_capacity(entries.len());
        let mut values = Vec::with_capacity(entries.len());
        let mut index = HashMap::with_capacity(entries.len());
        for (raw, (id, value)) in entries.into_iter().enumerate() {
            // Длина проверена выше, поэтому raw заведомо влезает в RawId.
            let raw = RawId::new(raw as u32);
            index.insert(id.clone(), raw);
            ids.push(id);
            values.push(value);
        }

        debug!(
            registry = self.name,
            entries = ids.len(),
            "реестр заморожен"
        );
        Ok(Frozen {
            name: self.name,
            ids: ids.into_boxed_slice(),
            values: values.into_boxed_slice(),
            index,
        })
    }
}

/// Замороженный реестр: числовые ID выданы, состав неизменен.
#[derive(Debug)]
pub struct Frozen<T> {
    name: &'static str,
    ids: Box<[Identifier]>,
    values: Box<[T]>,
    index: HashMap<Identifier, RawId>,
}

impl<T> Frozen<T> {
    /// Имя реестра.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Сколько записей.
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Пуст ли реестр.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Числовой ID по текстовому.
    #[inline]
    pub fn raw_id(&self, id: &Identifier) -> Option<RawId> {
        self.index.get(id).copied()
    }

    /// Значение по числовому ID.
    #[inline]
    pub fn get(&self, raw: RawId) -> Option<&T> {
        self.values.get(raw.index())
    }

    /// Текстовый ID по числовому.
    #[inline]
    pub fn identifier(&self, raw: RawId) -> Option<&Identifier> {
        self.ids.get(raw.index())
    }

    /// Значение по текстовому ID.
    pub fn get_by_id(&self, id: &Identifier) -> Option<&T> {
        self.raw_id(id).and_then(|raw| self.get(raw))
    }

    /// Все записи по возрастанию числового ID.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (RawId, &Identifier, &T)> {
        self.ids
            .iter()
            .zip(self.values.iter())
            .enumerate()
            .map(|(raw, (id, value))| (RawId::new(raw as u32), id, value))
    }

    /// Значения по возрастанию числового ID.
    pub fn values(&self) -> &[T] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("корректный id")
    }

    fn заполнить(order: &[&str]) -> Frozen<String> {
        let mut registry = Registry::new("test");
        for text in order {
            registry
                .register(id(text), (*text).to_owned())
                .expect("без дублей");
        }
        registry.freeze().expect("заморозка удалась")
    }

    #[test]
    fn регистрация_и_поиск() {
        let mut registry = Registry::new("blocks");
        registry.register(id("base:stone"), 1u32).expect("ok");
        registry.register(id("base:dirt"), 2u32).expect("ok");
        assert_eq!(registry.len(), 2);
        assert!(registry.contains(&id("base:stone")));
        assert!(!registry.contains(&id("base:sand")));

        let frozen = registry.freeze().expect("заморозка удалась");
        assert_eq!(frozen.get_by_id(&id("base:stone")), Some(&1));
        assert_eq!(frozen.get_by_id(&id("base:dirt")), Some(&2));
        assert_eq!(frozen.get_by_id(&id("base:sand")), None);
    }

    #[test]
    fn дубликат_отвергается() {
        let mut registry = Registry::new("blocks");
        registry.register(id("base:stone"), 1u32).expect("ok");
        let err = registry
            .register(id("base:stone"), 2u32)
            .expect_err("дубликат должен быть ошибкой");
        assert!(matches!(err, RegistryError::Duplicate { .. }));
        let frozen = registry.freeze().expect("заморозка удалась");
        assert_eq!(
            frozen.get_by_id(&id("base:stone")),
            Some(&1),
            "первое значение не должно перезаписываться"
        );
    }

    #[test]
    fn числовые_id_не_зависят_от_порядка_регистрации() {
        let a = заполнить(&["base:stone", "base:dirt", "mod:copper"]);
        let b = заполнить(&["mod:copper", "base:stone", "base:dirt"]);
        let c = заполнить(&["base:dirt", "mod:copper", "base:stone"]);

        for frozen in [&a, &b, &c] {
            assert_eq!(frozen.raw_id(&id("base:dirt")), Some(RawId::new(0)));
            assert_eq!(frozen.raw_id(&id("base:stone")), Some(RawId::new(1)));
            assert_eq!(frozen.raw_id(&id("mod:copper")), Some(RawId::new(2)));
        }
    }

    #[test]
    fn числовой_и_текстовый_id_переводятся_друг_в_друга() {
        let frozen = заполнить(&["base:stone", "base:dirt"]);
        for (raw, text, _) in frozen.iter() {
            assert_eq!(frozen.raw_id(text), Some(raw));
            assert_eq!(frozen.identifier(raw), Some(text));
        }
    }

    #[test]
    fn id_за_границей_даёт_none() {
        let frozen = заполнить(&["base:stone"]);
        assert_eq!(frozen.len(), 1);
        assert!(frozen.get(RawId::new(0)).is_some());
        assert!(frozen.get(RawId::new(1)).is_none());
        assert!(frozen.identifier(RawId::new(1)).is_none());
    }

    #[test]
    fn обход_идёт_по_возрастанию_id() {
        let frozen = заполнить(&["mod:copper", "base:stone", "base:dirt"]);
        let order: Vec<&str> = frozen.iter().map(|(_, id, _)| id.as_str()).collect();
        assert_eq!(order, ["base:dirt", "base:stone", "mod:copper"]);
        let raws: Vec<u32> = frozen.iter().map(|(raw, _, _)| raw.get()).collect();
        assert_eq!(raws, [0, 1, 2]);
    }

    #[test]
    fn пустой_реестр_замораживается() {
        let registry: Registry<u32> = Registry::new("empty");
        assert!(registry.is_empty());
        let frozen = registry.freeze().expect("заморозка удалась");
        assert!(frozen.is_empty());
        assert_eq!(frozen.len(), 0);
    }
}
