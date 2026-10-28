//! Этапы кадра и тика.

use bevy_ecs::schedule::ScheduleLabel;

/// Этап, в который плагин добавляет свои системы.
///
/// Порядок внутри кадра задан [`Stage::FRAME_ORDER`]. [`Stage::Startup`] выполняется
/// один раз перед первым кадром, [`Stage::FixedTick`] — ноль или больше раз за кадр
/// (см. [`crate::FixedTick`]).
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Один раз перед первым кадром: загрузка контента, заморозка реестров.
    Startup,
    /// Начало кадра: обновление времени и буферов сообщений. Служебный этап.
    First,
    /// Подготовка ввода и внешних данных перед логикой.
    PreUpdate,
    /// Фиксированный тик симуляции (по умолчанию 20 Hz). Вся логика мира — здесь.
    FixedTick,
    /// Логика, привязанная к кадру, а не к тику: камера, свободный полёт.
    Update,
    /// Реакция на изменения этого кадра.
    PostUpdate,
    /// Подготовка данных для рендера: стриминг, culling, загрузка в GPU.
    PreRender,
    /// Отрисовка кадра.
    Render,
    /// Конец кадра: метрики, очистка.
    Last,
}

impl Stage {
    /// Этапы в порядке выполнения внутри кадра. [`Stage::FixedTick`] исключён:
    /// им управляет аккумулятор фиксированного тика.
    pub const FRAME_ORDER: &'static [Stage] = &[
        Stage::First,
        Stage::PreUpdate,
        Stage::Update,
        Stage::PostUpdate,
        Stage::PreRender,
        Stage::Render,
        Stage::Last,
    ];

    /// Все этапы, включая [`Stage::Startup`] и [`Stage::FixedTick`].
    pub const ALL: &'static [Stage] = &[
        Stage::Startup,
        Stage::First,
        Stage::PreUpdate,
        Stage::FixedTick,
        Stage::Update,
        Stage::PostUpdate,
        Stage::PreRender,
        Stage::Render,
        Stage::Last,
    ];
}
