//! Базовый контент игры: блоки пространства имён `base`.
//!
//! Крейт пользуется только публичным API движка — ровно так же, как будущий мод.
//! Никаких внутренних хаков для базового контента здесь нет и быть не должно:
//! если чего-то не хватает, это дыра в API движка, а не повод её обойти.

use bevy_ecs::prelude::*;
use engine_assets::{AssetError, ContentRoot, load_blocks};
use engine_core::{App, AppExit, Plugin, Stage};
use engine_world::{BlockRegistry, ContentSet};
use tracing::{error, info};

/// Пространство имён базового контента.
pub const NAMESPACE: &str = "base";

/// Плагин базового контента.
///
/// Блоки описаны данными в `assets/base/blocks/*.ron`, а не кодом: добавление
/// блока — это новый файл и текстура, без правок движка и без правок этого крейта.
#[derive(Debug, Clone, Copy, Default)]
pub struct BasePlugin;

impl Plugin for BasePlugin {
    fn name(&self) -> &'static str {
        "game_base"
    }

    fn build(&self, app: &mut App) {
        app.add_systems(Stage::Startup, register_blocks.in_set(ContentSet::Register));
    }
}

/// Прочитать описания блоков и зарегистрировать их.
fn register_blocks(
    root: Res<ContentRoot>,
    mut registry: ResMut<BlockRegistry>,
    mut exit: MessageWriter<AppExit>,
) {
    let blocks = match load_blocks(&root, NAMESPACE) {
        Ok(blocks) => blocks,
        Err(err) => {
            fail(&mut exit, &err);
            return;
        }
    };

    if blocks.is_empty() {
        error!(
            dir = %root.blocks_dir(NAMESPACE).display(),
            "не найдено ни одного описания блока: запущено не из корня проекта?"
        );
        exit.write(AppExit);
        return;
    }

    let mut registered = 0;
    for block in &blocks {
        if let Err(err) = registry.register(block.id.clone(), block.definition.properties()) {
            error!(
                id = %block.id,
                source = %block.source.display(),
                %err,
                "не удалось зарегистрировать блок"
            );
            exit.write(AppExit);
            return;
        }
        registered += 1;
    }

    info!(
        namespace = NAMESPACE,
        blocks = registered,
        "контент зарегистрирован"
    );
}

/// Сообщить об ошибке контента и попросить выход.
///
/// Паника здесь оставила бы пользователя с backtrace вместо имени файла,
/// в котором опечатка.
fn fail(exit: &mut MessageWriter<AppExit>, err: &AssetError) {
    error!(%err, "не удалось загрузить базовый контент");
    exit.write(AppExit);
}
