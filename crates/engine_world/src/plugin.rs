//! Плагин мира: реестр блоков, его заморозка и ресурс мира.

use crate::{BlockRegistry, Blocks, VoxelWorld};
use bevy_ecs::prelude::*;
use engine_core::{App, AppExit, Plugin, Stage};
use tracing::{error, info};

/// Группы систем этапа [`Stage::Startup`], задающие порядок работы с контентом.
///
/// Плагины контента не знают друг о друге и не могут сослаться на чужую систему,
/// поэтому порядок задаётся группами: всё, что регистрирует контент, идёт в
/// [`ContentSet::Register`], и только после этого реестры замораживаются.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentSet {
    /// Регистрация контента: блоки, предметы, генераторы.
    Register,
    /// Заморозка реестров и выдача числовых ID.
    Freeze,
}

/// Плагин мира.
///
/// Даёт реестр блоков для наполнения, мир как ресурс и систему заморозки.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn name(&self) -> &'static str {
        "engine_world"
    }

    fn build(&self, app: &mut App) {
        app.insert_resource(BlockRegistry::new());
        app.init_resource::<VoxelWorld>();
        app.configure_sets(
            Stage::Startup,
            ContentSet::Register.before(ContentSet::Freeze),
        );
        app.add_systems(
            Stage::Startup,
            freeze_block_registry.in_set(ContentSet::Freeze),
        );
    }
}

/// Заморозить реестр блоков: убрать его из мира и положить вместо него [`Blocks`].
///
/// Эксклюзивная система: нужно забрать ресурс целиком, а не одолжить ссылку —
/// заморозка потребляет реестр, чтобы после неё в него нельзя было дописать.
fn freeze_block_registry(world: &mut World) {
    let Some(registry) = world.remove_resource::<BlockRegistry>() else {
        error!("реестр блоков отсутствует: WorldPlugin установлен дважды?");
        return;
    };

    match registry.freeze() {
        Ok(blocks) => {
            info!(blocks = blocks.len(), "реестр блоков заморожен");
            world.insert_resource(BlocksResource(blocks));
        }
        Err(err) => {
            // Без блоков движок бесполезен, но паника здесь оставила бы
            // пользователя с backtrace вместо внятного сообщения.
            error!(%err, "не удалось заморозить реестр блоков");
            if let Some(mut exit) = world.get_resource_mut::<bevy_ecs::message::Messages<AppExit>>()
            {
                exit.write(AppExit);
            }
        }
    }
}

/// Замороженный реестр блоков как ресурс.
#[derive(Debug, Resource)]
pub struct BlocksResource(pub Blocks);

impl std::ops::Deref for BlocksResource {
    type Target = Blocks;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AIR_ID, BlockProperties};
    use engine_registry::Identifier;

    fn id(text: &str) -> Identifier {
        Identifier::parse(text).expect("корректный id")
    }

    /// Плагин, регистрирующий блоки — как это делает game_base.
    struct TestContent;
    impl Plugin for TestContent {
        fn name(&self) -> &'static str {
            "test_content"
        }
        fn build(&self, app: &mut App) {
            app.add_systems(
                Stage::Startup,
                register_test_blocks.in_set(ContentSet::Register),
            );
        }
    }

    fn register_test_blocks(mut registry: ResMut<BlockRegistry>) {
        registry
            .register(id(AIR_ID), BlockProperties::AIR)
            .expect("воздух регистрируется");
        registry
            .register(id("test:stone"), BlockProperties::SOLID)
            .expect("камень регистрируется");
    }

    #[test]
    fn регистрация_происходит_до_заморозки() {
        let mut app = App::new();
        app.add_plugin(WorldPlugin);
        app.add_plugin(TestContent);
        app.update();

        let blocks = app
            .world
            .get_resource::<BlocksResource>()
            .expect("реестр должен быть заморожен");
        assert_eq!(blocks.len(), 2);
        assert!(blocks.id(&id("test:stone")).is_some());
        assert!(
            app.world.get_resource::<BlockRegistry>().is_none(),
            "после заморозки дописать в реестр нельзя"
        );
    }

    #[test]
    fn порядок_установки_плагинов_не_важен() {
        // Контент установлен раньше мира — группы систем всё равно упорядочены.
        let mut app = App::new();
        app.add_plugin(TestContent);
        app.add_plugin(WorldPlugin);
        app.update();

        let blocks = app
            .world
            .get_resource::<BlocksResource>()
            .expect("реестр должен быть заморожен");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn без_воздуха_приложение_просит_выход_а_не_падает() {
        struct NoAir;
        impl Plugin for NoAir {
            fn name(&self) -> &'static str {
                "no_air"
            }
            fn build(&self, app: &mut App) {
                app.add_systems(
                    Stage::Startup,
                    (|mut registry: ResMut<BlockRegistry>| {
                        registry
                            .register(id("test:stone"), BlockProperties::SOLID)
                            .expect("камень регистрируется");
                    })
                    .in_set(ContentSet::Register),
                );
            }
        }

        let mut app = App::new();
        app.add_plugin(WorldPlugin);
        app.add_plugin(NoAir);
        app.update();

        assert!(app.world.get_resource::<BlocksResource>().is_none());
        assert!(app.exit_requested(), "движок должен попросить выход");
    }

    #[test]
    fn мир_доступен_как_ресурс() {
        let mut app = App::new();
        app.add_plugin(WorldPlugin);
        app.add_plugin(TestContent);
        app.update();

        let world = app
            .world
            .get_resource::<VoxelWorld>()
            .expect("мир должен быть ресурсом");
        assert!(world.is_empty());
    }
}
