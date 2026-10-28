//! Бинарник движка: собирает `App` из плагинов и запускает.
//!
//! Здесь нет логики — только перечисление плагинов. Любое поведение движка
//! и игры добавляется плагином, в том числе базовый контент (`game_base`).

use anyhow::Result;
use bevy_ecs::prelude::*;
use engine_assets::AssetsPlugin;
use engine_core::{App, Stage};
use engine_world::{BlocksResource, ContentSet, WorldPlugin};
use game_base::BasePlugin;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    init_tracing();

    let mut app = App::new();
    app.add_plugin(AssetsPlugin::new());
    app.add_plugin(WorldPlugin);
    app.add_plugin(BasePlugin);
    // Дальше по мере появления:
    //   app.add_plugin(engine_platform::PlatformPlugin);
    //   app.add_plugin(engine_render::RenderPlugin);
    //   app.add_plugin(engine_worldgen::WorldgenPlugin);

    app.add_systems(Stage::Startup, log_blocks.after(ContentSet::Freeze));

    info!(plugins = ?app.plugins(), "приложение собрано");
    app.run();
    Ok(())
}

/// Логи через `tracing`. Уровень задаётся `RUST_LOG`, по умолчанию `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}

/// Показать, какие блоки зарегистрированы и какие числовые ID получили.
///
/// Пока нет окна, это единственный видимый результат запуска. Когда появится
/// рендер, останется как диагностика на уровне `debug`.
fn log_blocks(blocks: Option<Res<BlocksResource>>) {
    let Some(blocks) = blocks else {
        return;
    };
    info!(count = blocks.len(), "блоки зарегистрированы");
    for (id, identifier, properties) in blocks.iter() {
        info!(
            id = id.get(),
            %identifier,
            opaque = properties.opaque,
            empty = properties.empty,
            "блок"
        );
    }
}
