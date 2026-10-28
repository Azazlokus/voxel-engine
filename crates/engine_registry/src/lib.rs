//! Текстовые идентификаторы контента и реестры.
//!
//! Блоки, предметы и генераторы регистрируются под текстовым ID вида `base:stone`.
//! Числовые ID выдаются один раз, при заморозке реестра после загрузки всех
//! плагинов, и нигде не хардкодятся — иначе набор контента зависел бы от порядка
//! установки плагинов, а мод не мог бы добавить свой блок.

mod identifier;
mod raw_id;
mod registry;

pub use identifier::{Identifier, IdentifierError, MAX_NAMESPACE_LEN, MAX_PATH_LEN};
pub use raw_id::RawId;
pub use registry::{Frozen, Registry, RegistryError};
