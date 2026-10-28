//! Тесты базового контента на настоящих файлах из `assets/base`.
//!
//! Смысл в том, чтобы ловить ошибки в самих данных: опечатку в RON, забытую
//! текстуру, переименованный блок. Тест на выдуманных файлах этого не поймал бы.

use engine_assets::{AssetsPlugin, ContentRoot, load_blocks};
use engine_core::App;
use engine_world::{AIR_ID, BlockId, BlocksResource, WorldPlugin};
use game_base::{BasePlugin, NAMESPACE};
use std::path::{Path, PathBuf};

/// Корень контента репозитория.
fn assets_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("у game_base есть родительская папка")
        .join("assets")
}

/// Собранное приложение с базовым контентом.
fn app_with_content() -> App {
    let mut app = App::new();
    app.add_plugin(AssetsPlugin::with_root(assets_root()));
    app.add_plugin(WorldPlugin);
    app.add_plugin(BasePlugin);
    app.update();
    app
}

#[test]
fn базовые_блоки_регистрируются() {
    let app = app_with_content();
    let blocks = app
        .world
        .get_resource::<BlocksResource>()
        .expect("реестр блоков должен быть заморожен");

    let expected = [
        "base:air",
        "base:dirt",
        "base:grass",
        "base:leaves",
        "base:log",
        "base:sand",
        "base:stone",
        "base:water",
    ];
    assert_eq!(blocks.len(), expected.len());
    for name in expected {
        let id = engine_registry::Identifier::parse(name).expect("корректный id");
        assert!(
            blocks.id(&id).is_some(),
            "{name} должен быть зарегистрирован"
        );
    }
    assert!(
        !app.exit_requested(),
        "загрузка контента не должна просить выход"
    );
}

#[test]
fn воздух_получает_нулевой_id() {
    let app = app_with_content();
    let blocks = app
        .world
        .get_resource::<BlocksResource>()
        .expect("реестр заморожен");
    let air = engine_registry::Identifier::parse(AIR_ID).expect("корректный id");
    assert_eq!(blocks.id(&air), Some(BlockId::AIR));
    assert!(blocks.is_empty_block(BlockId::AIR));
    assert!(!blocks.is_opaque(BlockId::AIR));
}

#[test]
fn свойства_блоков_осмысленны() {
    let app = app_with_content();
    let blocks = app
        .world
        .get_resource::<BlocksResource>()
        .expect("реестр заморожен");

    for (id, identifier, properties) in blocks.iter() {
        if identifier.as_str() == AIR_ID {
            continue;
        }
        assert!(
            properties.opaque,
            "{identifier} должен быть непрозрачным в задаче 01 (см. ADR 0003)"
        );
        assert!(!properties.empty, "{identifier} не пустой");
        assert!(!id.is_air(), "{identifier} не должен получить ID воздуха");
    }
}

#[test]
fn у_каждого_непустого_блока_есть_файлы_текстур() {
    // Описание может ссылаться на текстуру, которой нет на диске: рендер узнает
    // об этом только на седьмом шаге, а данные неверны уже сейчас.
    let root = ContentRoot::new(assets_root());
    let blocks = load_blocks(&root, NAMESPACE).expect("описания читаются");
    let textures_dir = root.textures_dir(NAMESPACE);

    for block in &blocks {
        for texture in block.definition.textures() {
            let id = engine_registry::Identifier::parse(texture).expect("корректный id");
            assert_eq!(
                id.namespace(),
                NAMESPACE,
                "{} ссылается на чужое пространство имён",
                block.id
            );
            let path = textures_dir.join(format!("{}.png", id.path()));
            assert!(
                path.is_file(),
                "для {} нет файла текстуры {}",
                block.id,
                path.display()
            );
        }
    }
}

#[test]
fn без_контента_приложение_просит_выход() {
    // Запуск не из корня проекта: папки assets нет.
    let mut app = App::new();
    app.add_plugin(AssetsPlugin::with_root(
        std::env::temp_dir().join("voxel-engine-no-such-assets"),
    ));
    app.add_plugin(WorldPlugin);
    app.add_plugin(BasePlugin);
    app.update();

    assert!(
        app.exit_requested(),
        "без контента движок должен сообщить и выйти, а не работать с пустым реестром"
    );
}
