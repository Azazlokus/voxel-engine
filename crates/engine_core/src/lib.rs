//! Ядро движка: приложение, плагины, этапы кадра и фиксированный тик.
//!
//! Всё в движке — плагин: рендер, мир, генерация, базовый контент игры.
//! `client` только перечисляет плагины, порядок установки не влияет на поведение
//! (связи между системами задаются этапами, а не порядком плагинов).

mod app;
mod plugin;
mod stage;
mod time;

pub use app::{App, AppExit};
pub use plugin::Plugin;
pub use stage::Stage;
pub use time::{FixedTick, Time};

/// Типы, нужные почти любому плагину.
pub mod prelude {
    pub use crate::{App, AppExit, FixedTick, Plugin, Stage, Time};
    pub use bevy_ecs::prelude::*;
}
