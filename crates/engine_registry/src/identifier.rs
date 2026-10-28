//! Текстовый идентификатор контента: `namespace:path`.

use core::fmt;
use thiserror::Error;

/// Максимальная длина пространства имён.
pub const MAX_NAMESPACE_LEN: usize = 64;
/// Максимальная длина пути.
pub const MAX_PATH_LEN: usize = 128;

/// Почему строка не является корректным идентификатором.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IdentifierError {
    /// В строке нет двоеточия: пространство имён обязательно.
    #[error("нет двоеточия в {0:?}: идентификатор должен иметь вид namespace:path")]
    MissingNamespace(String),

    /// Двоеточий больше одного.
    #[error("больше одного двоеточия в {0:?}")]
    TooManyColons(String),

    /// Пространство имён пусто.
    #[error("пустое пространство имён в {0:?}")]
    EmptyNamespace(String),

    /// Путь пуст.
    #[error("пустой путь в {0:?}")]
    EmptyPath(String),

    /// Недопустимый символ.
    #[error("недопустимый символ {ch:?} в {part} идентификатора {full:?}")]
    InvalidChar {
        /// Сам символ.
        ch: char,
        /// `пространстве имён` или `пути`.
        part: &'static str,
        /// Исходная строка.
        full: String,
    },

    /// Слишком длинно.
    #[error("{part} длиной {len} превышает предел {max} в {full:?}")]
    TooLong {
        /// Какая часть превышена.
        part: &'static str,
        /// Фактическая длина.
        len: usize,
        /// Предел.
        max: usize,
        /// Исходная строка.
        full: String,
    },

    /// Путь содержит пустой сегмент: ведущий, замыкающий или двойной слэш.
    #[error("пустой сегмент пути в {0:?}")]
    EmptyPathSegment(String),
}

/// Идентификатор контента вида `base:stone`.
///
/// Хранится одной строкой вместе с двоеточием: сравнение, хеширование и вывод
/// работают с готовым текстом, без склейки частей на каждый вызов.
///
/// Порядок — лексикографический по полному тексту. На нём держится выдача
/// числовых ID при заморозке реестра: она не зависит от порядка регистрации.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
    text: Box<str>,
    /// Позиция двоеточия. Всегда внутри `text`.
    colon: u32,
}

impl Identifier {
    /// Собрать из частей.
    pub fn new(namespace: &str, path: &str) -> Result<Self, IdentifierError> {
        let full = format!("{namespace}:{path}");
        Self::validate(namespace, path, &full)?;
        Ok(Self {
            colon: namespace.len() as u32,
            text: full.into_boxed_str(),
        })
    }

    /// Разобрать строку `namespace:path`.
    pub fn parse(text: &str) -> Result<Self, IdentifierError> {
        let mut parts = text.split(':');
        let namespace = parts.next().unwrap_or_default();
        let path = match parts.next() {
            Some(path) => path,
            None => return Err(IdentifierError::MissingNamespace(text.to_owned())),
        };
        if parts.next().is_some() {
            return Err(IdentifierError::TooManyColons(text.to_owned()));
        }
        Self::validate(namespace, path, text)?;
        Ok(Self {
            colon: namespace.len() as u32,
            text: text.to_owned().into_boxed_str(),
        })
    }

    /// Пространство имён: что до двоеточия.
    #[inline]
    pub fn namespace(&self) -> &str {
        &self.text[..self.colon as usize]
    }

    /// Путь: что после двоеточия.
    #[inline]
    pub fn path(&self) -> &str {
        &self.text[self.colon as usize + 1..]
    }

    /// Полный текст `namespace:path`.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    fn validate(namespace: &str, path: &str, full: &str) -> Result<(), IdentifierError> {
        if namespace.is_empty() {
            return Err(IdentifierError::EmptyNamespace(full.to_owned()));
        }
        if path.is_empty() {
            return Err(IdentifierError::EmptyPath(full.to_owned()));
        }
        if namespace.len() > MAX_NAMESPACE_LEN {
            return Err(IdentifierError::TooLong {
                part: "пространство имён",
                len: namespace.len(),
                max: MAX_NAMESPACE_LEN,
                full: full.to_owned(),
            });
        }
        if path.len() > MAX_PATH_LEN {
            return Err(IdentifierError::TooLong {
                part: "путь",
                len: path.len(),
                max: MAX_PATH_LEN,
                full: full.to_owned(),
            });
        }
        for ch in namespace.chars() {
            if !is_namespace_char(ch) {
                return Err(IdentifierError::InvalidChar {
                    ch,
                    part: "пространстве имён",
                    full: full.to_owned(),
                });
            }
        }
        for ch in path.chars() {
            if !is_path_char(ch) {
                return Err(IdentifierError::InvalidChar {
                    ch,
                    part: "пути",
                    full: full.to_owned(),
                });
            }
        }
        // Пустые сегменты запрещены: "base:/stone", "base:stone/" и "base:a//b"
        // различались бы только незначащим слэшем и путали бы поиск файлов.
        if path.starts_with('/') || path.ends_with('/') || path.contains("//") {
            return Err(IdentifierError::EmptyPathSegment(full.to_owned()));
        }
        Ok(())
    }
}

/// Допустимые символы пространства имён: строчные буквы, цифры, `_`, `.`, `-`.
#[inline]
const fn is_namespace_char(ch: char) -> bool {
    matches!(ch, 'a'..='z' | '0'..='9' | '_' | '.' | '-')
}

/// Допустимые символы пути: как в пространстве имён плюс `/` для вложенности.
#[inline]
const fn is_path_char(ch: char) -> bool {
    is_namespace_char(ch) || ch == '/'
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Отладочный вывод читается как исходная строка, а не как структура.
        write!(f, "{:?}", &*self.text)
    }
}

impl core::str::FromStr for Identifier {
    type Err = IdentifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn разбор_и_части() {
        let id = Identifier::parse("base:stone").expect("корректный id");
        assert_eq!(id.namespace(), "base");
        assert_eq!(id.path(), "stone");
        assert_eq!(id.as_str(), "base:stone");
        assert_eq!(id.to_string(), "base:stone");
    }

    #[test]
    fn вложенный_путь_разрешён() {
        let id = Identifier::parse("base:blocks/ore/iron").expect("корректный id");
        assert_eq!(id.path(), "blocks/ore/iron");
    }

    #[test]
    fn сборка_из_частей_равна_разбору() {
        let a = Identifier::new("base", "stone").expect("корректный id");
        let b = Identifier::parse("base:stone").expect("корректный id");
        assert_eq!(a, b);
    }

    #[test]
    fn без_двоеточия_отвергается() {
        assert!(matches!(
            Identifier::parse("stone"),
            Err(IdentifierError::MissingNamespace(_))
        ));
    }

    #[test]
    fn два_двоеточия_отвергаются() {
        assert!(matches!(
            Identifier::parse("base:stone:extra"),
            Err(IdentifierError::TooManyColons(_))
        ));
    }

    #[test]
    fn пустые_части_отвергаются() {
        assert!(matches!(
            Identifier::parse(":stone"),
            Err(IdentifierError::EmptyNamespace(_))
        ));
        assert!(matches!(
            Identifier::parse("base:"),
            Err(IdentifierError::EmptyPath(_))
        ));
    }

    #[test]
    fn заглавные_буквы_и_пробелы_отвергаются() {
        // Иначе base:Stone и base:stone были бы разными блоками, неразличимыми
        // на глаз в логах и именах файлов.
        assert!(matches!(
            Identifier::parse("base:Stone"),
            Err(IdentifierError::InvalidChar { ch: 'S', .. })
        ));
        assert!(matches!(
            Identifier::parse("base:red stone"),
            Err(IdentifierError::InvalidChar { ch: ' ', .. })
        ));
        assert!(matches!(
            Identifier::parse("Base:stone"),
            Err(IdentifierError::InvalidChar { ch: 'B', .. })
        ));
    }

    #[test]
    fn слэш_в_пространстве_имён_отвергается() {
        assert!(matches!(
            Identifier::parse("base/mod:stone"),
            Err(IdentifierError::InvalidChar { ch: '/', .. })
        ));
    }

    #[test]
    fn пустой_сегмент_пути_отвергается() {
        for bad in ["base:/stone", "base:stone/", "base:a//b"] {
            assert!(
                matches!(
                    Identifier::parse(bad),
                    Err(IdentifierError::EmptyPathSegment(_))
                ),
                "{bad} должен быть отвергнут"
            );
        }
    }

    #[test]
    fn слишком_длинные_части_отвергаются() {
        let long_ns = "a".repeat(MAX_NAMESPACE_LEN + 1);
        assert!(matches!(
            Identifier::parse(&format!("{long_ns}:stone")),
            Err(IdentifierError::TooLong { .. })
        ));
        let long_path = "a".repeat(MAX_PATH_LEN + 1);
        assert!(matches!(
            Identifier::parse(&format!("base:{long_path}")),
            Err(IdentifierError::TooLong { .. })
        ));
        let max_ns = "a".repeat(MAX_NAMESPACE_LEN);
        assert!(Identifier::parse(&format!("{max_ns}:stone")).is_ok());
    }

    #[test]
    fn порядок_лексикографический_по_полному_тексту() {
        let mut ids = [
            Identifier::parse("mod:apple").expect("корректный id"),
            Identifier::parse("base:stone").expect("корректный id"),
            Identifier::parse("base:dirt").expect("корректный id"),
        ];
        ids.sort();
        let order: Vec<&str> = ids.iter().map(Identifier::as_str).collect();
        assert_eq!(order, ["base:dirt", "base:stone", "mod:apple"]);
    }
}
