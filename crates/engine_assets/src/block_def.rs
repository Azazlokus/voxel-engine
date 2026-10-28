//! Описание блока в RON.

use engine_world::{BlockProperties, Face};
use serde::Deserialize;

/// Описание блока из `assets/<namespace>/blocks/<path>.ron`.
///
/// Это полное описание контента, в отличие от [`BlockProperties`], куда попадает
/// только нужное миру и мешеру. Текстуры и твёрдость читает рендер и игровая логика.
///
/// Пример файла `assets/base/blocks/grass.ron`:
///
/// ```ron
/// (
///     textures: Layered(
///         top: "base:grass_top",
///         side: "base:grass_side",
///         bottom: "base:dirt",
///     ),
///     hardness: 0.6,
/// )
/// ```
#[derive(Debug, Clone, Deserialize)]
// Опечатка в имени поля должна быть ошибкой, а не молчаливым значением
// по умолчанию: иначе `opacue: false` тихо даст непрозрачный блок.
#[serde(deny_unknown_fields)]
pub struct BlockDefinition {
    /// Текстуры граней. Отсутствуют только у пустых блоков вроде воздуха —
    /// придумывать им фальшивую текстуру незачем. Для непустого блока
    /// отсутствие текстур проверяется загрузчиком.
    #[serde(default)]
    pub textures: Option<TextureSet>,

    /// Полностью перекрывает обзор. По умолчанию да.
    #[serde(default = "default_opaque")]
    pub opaque: bool,

    /// Блок без геометрии и препятствия (воздух). По умолчанию нет.
    #[serde(default)]
    pub empty: bool,

    /// Сколько времени занимает разрушение. Отрицательное — неразрушимый.
    /// Пока не используется: ломание блоков вне скоупа задачи 01.
    #[serde(default)]
    pub hardness: f32,

    /// Собственная светимость, `0..=15`.
    #[serde(default)]
    pub light: u8,
}

fn default_opaque() -> bool {
    true
}

impl BlockDefinition {
    /// Свойства, которые нужны миру и мешеру.
    pub fn properties(&self) -> BlockProperties {
        BlockProperties {
            // Пустой блок не может быть непрозрачным: иначе мешер не нарисует
            // грани соседей, и внутри воздуха появится дыра в геометрии.
            opaque: self.opaque && !self.empty,
            light: self.light.min(15),
            empty: self.empty,
        }
    }

    /// Текстура грани. `None` у блоков без текстур.
    pub fn texture(&self, face: Face) -> Option<&str> {
        self.textures.as_ref().map(|set| set.texture(face))
    }

    /// Все упомянутые текстуры без повторов, в порядке первого упоминания.
    pub fn textures(&self) -> Vec<&str> {
        let Some(set) = self.textures.as_ref() else {
            return Vec::new();
        };
        let mut out: Vec<&str> = Vec::with_capacity(3);
        for face in Face::ALL {
            let texture = set.texture(face);
            if !out.contains(&texture) {
                out.push(texture);
            }
        }
        out
    }

    /// Нужны ли этому блоку текстуры.
    pub fn needs_textures(&self) -> bool {
        !self.empty
    }
}

/// Какие текстуры у граней блока.
#[derive(Debug, Clone, Deserialize)]
pub enum TextureSet {
    /// Одна текстура на все шесть граней.
    All(String),

    /// Верх, бока, низ — самый частый случай для земли и травы.
    Layered {
        /// Верхняя грань.
        top: String,
        /// Четыре боковые грани.
        side: String,
        /// Нижняя грань.
        bottom: String,
    },

    /// Своя текстура каждой грани.
    PerFace {
        /// Грань в сторону +X.
        pos_x: String,
        /// Грань в сторону -X.
        neg_x: String,
        /// Верхняя грань.
        pos_y: String,
        /// Нижняя грань.
        neg_y: String,
        /// Грань в сторону +Z.
        pos_z: String,
        /// Грань в сторону -Z.
        neg_z: String,
    },
}

impl TextureSet {
    /// Текстура грани.
    pub fn texture(&self, face: Face) -> &str {
        match self {
            TextureSet::All(texture) => texture,
            TextureSet::Layered { top, side, bottom } => {
                if face.is_top() {
                    top
                } else if face.is_bottom() {
                    bottom
                } else {
                    side
                }
            }
            TextureSet::PerFace {
                pos_x,
                neg_x,
                pos_y,
                neg_y,
                pos_z,
                neg_z,
            } => match face {
                Face::PosX => pos_x,
                Face::NegX => neg_x,
                Face::PosY => pos_y,
                Face::NegY => neg_y,
                Face::PosZ => pos_z,
                Face::NegZ => neg_z,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn разобрать(text: &str) -> BlockDefinition {
        crate::loader::ron_options()
            .from_str(text)
            .expect("корректный RON")
    }

    fn разобрать_результат(
        text: &str,
    ) -> Result<BlockDefinition, ron::error::SpannedError> {
        crate::loader::ron_options().from_str(text)
    }

    #[test]
    fn одна_текстура_на_все_грани() {
        let def = разобрать(r#"(textures: All("base:stone"))"#);
        for face in Face::ALL {
            assert_eq!(def.texture(face), Some("base:stone"), "{face:?}");
        }
        assert_eq!(def.textures(), vec!["base:stone"]);
    }

    #[test]
    fn слои_верх_бока_низ() {
        let def = разобрать(
            r#"(textures: Layered(top: "base:grass_top", side: "base:grass_side", bottom: "base:dirt"))"#,
        );
        assert_eq!(def.texture(Face::PosY), Some("base:grass_top"));
        assert_eq!(def.texture(Face::NegY), Some("base:dirt"));
        for face in [Face::PosX, Face::NegX, Face::PosZ, Face::NegZ] {
            assert_eq!(def.texture(face), Some("base:grass_side"), "{face:?}");
        }
        assert_eq!(
            def.textures(),
            vec!["base:grass_side", "base:grass_top", "base:dirt"]
        );
    }

    #[test]
    fn своя_текстура_каждой_грани() {
        let def = разобрать(
            r#"(textures: PerFace(
                pos_x: "a", neg_x: "b", pos_y: "c",
                neg_y: "d", pos_z: "e", neg_z: "f",
            ))"#,
        );
        assert_eq!(def.texture(Face::PosX), Some("a"));
        assert_eq!(def.texture(Face::NegX), Some("b"));
        assert_eq!(def.texture(Face::PosY), Some("c"));
        assert_eq!(def.texture(Face::NegY), Some("d"));
        assert_eq!(def.texture(Face::PosZ), Some("e"));
        assert_eq!(def.texture(Face::NegZ), Some("f"));
        assert_eq!(def.textures().len(), 6);
    }

    #[test]
    fn значения_по_умолчанию() {
        let def = разобрать(r#"(textures: All("base:stone"))"#);
        assert!(def.opaque, "блок непрозрачен по умолчанию");
        assert!(!def.empty);
        assert_eq!(def.light, 0);
        assert_eq!(def.hardness, 0.0);
    }

    #[test]
    fn свойства_для_мира() {
        let def = разобрать(r#"(textures: All("base:stone"), hardness: 1.5)"#);
        let properties = def.properties();
        assert!(properties.opaque);
        assert!(!properties.empty);

        let air = разобрать(r#"(textures: All("base:air"), opaque: false, empty: true)"#);
        assert_eq!(air.properties(), BlockProperties::AIR);
    }

    #[test]
    fn пустой_блок_не_может_быть_непрозрачным() {
        // Иначе мешер не нарисует грани соседей, и в воздухе появится дыра.
        let def = разобрать(r#"(textures: All("base:air"), opaque: true, empty: true)"#);
        assert!(!def.properties().opaque);
    }

    #[test]
    fn светимость_обрезается_до_предела() {
        let def = разобрать(r#"(textures: All("base:lamp"), light: 200)"#);
        assert_eq!(def.properties().light, 15);
    }

    #[test]
    fn неизвестное_поле_отвергается() {
        // Опечатка не должна тихо превращаться в значение по умолчанию.
        let err = разобрать_результат(
            r#"(textures: All("base:stone"), opacue: false)"#,
        );
        assert!(err.is_err(), "опечатка в имени поля должна быть ошибкой");
    }

    #[test]
    fn блок_без_текстур_разбирается_но_помечен_нуждающимся() {
        // Непустой блок без текстур разбирается, а отвергается загрузчиком:
        // там есть путь к файлу для внятного сообщения.
        let def = разобрать(r#"(hardness: 1.0)"#);
        assert!(def.textures.is_none());
        assert!(def.needs_textures(), "непустому блоку текстуры нужны");
        assert!(def.textures().is_empty());
        assert_eq!(def.texture(Face::PosY), None);

        let air = разобрать(r#"(empty: true, opaque: false)"#);
        assert!(!air.needs_textures(), "пустому блоку текстуры не нужны");
    }
}
