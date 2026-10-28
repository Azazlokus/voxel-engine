//! Бинарник движка: собирает `App` из плагинов и запускает.
//!
//! Здесь нет логики — только перечисление плагинов. Любое поведение движка
//! и игры добавляется плагином, в том числе базовый контент (`game_base`).

use anyhow::Result;
use engine_core::App;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    init_tracing();

    // `mut` появится вместе с первым плагином.
    let app = App::new();

    // Плагины подключаются здесь по мере появления:
    //   app.add_plugin(engine_platform::PlatformPlugin);
    //   app.add_plugin(engine_render::RenderPlugin);
    //   app.add_plugin(engine_world::WorldPlugin);
    //   app.add_plugin(game_base::BasePlugin);

    info!(plugins = ?app.plugins(), "приложение собрано");
    app.run();
    info!("приложение завершено");

    Ok(())
}

/// Логи через `tracing`. Уровень задаётся `RUST_LOG`, по умолчанию `info`.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
