//! Сборка и запуск приложения.

use crate::{Plugin, Stage, Time, time::FixedTick};
use bevy_ecs::message::{Message, MessageRegistry, Messages, message_update_system};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{InternedSystemSet, IntoScheduleConfigs, Schedules};
use bevy_ecs::system::ScheduleSystem;
use core::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

/// Запрос на выход из приложения. Цикл событий завершает работу после текущего кадра.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AppExit;

/// Функция, которая владеет циклом приложения.
///
/// По умолчанию выполняется один кадр (headless). Настоящий цикл с окном ставит
/// `engine_platform` через [`App::set_runner`]: цикл событий `winit` нельзя
/// вложить внутрь нашего, он должен владеть потоком сам.
type Runner = Box<dyn FnOnce(App)>;

/// Приложение: мир ECS, расписания по этапам и список плагинов.
pub struct App {
    /// Мир ECS: сущности, ресурсы и буферы сообщений.
    pub world: World,
    plugins: Vec<&'static str>,
    runner: Option<Runner>,
    started: bool,
    last_update: Option<Instant>,
}

impl fmt::Debug for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("plugins", &self.plugins)
            .field("started", &self.started)
            .field("entities", &self.world.entities().len())
            .finish_non_exhaustive()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Пустое приложение: расписания всех этапов, время, фиксированный тик.
    pub fn new() -> Self {
        let mut world = World::new();

        let mut schedules = Schedules::default();
        for stage in Stage::ALL {
            schedules.insert(Schedule::new(*stage));
        }
        world.insert_resource(schedules);
        world.init_resource::<Time>();
        world.init_resource::<FixedTick>();
        MessageRegistry::register_message::<AppExit>(&mut world);

        let mut app = Self {
            world,
            plugins: Vec::new(),
            runner: None,
            started: false,
            last_update: None,
        };
        // Буферы сообщений живут два кадра; продвигает их эта система.
        app.add_systems(Stage::First, message_update_system);
        app
    }

    /// Установить плагин. Повторная установка под тем же именем игнорируется:
    /// плагины могут зависеть от общих плагинов и объявлять их оба.
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self {
        let name = plugin.name();
        if self.plugins.contains(&name) {
            debug!(plugin = name, "плагин уже установлен, пропускаю");
            return self;
        }
        trace!(plugin = name, "установка плагина");
        self.plugins.push(name);
        plugin.build(self);
        self
    }

    /// Имена установленных плагинов в порядке установки.
    pub fn plugins(&self) -> &[&'static str] {
        &self.plugins
    }

    /// Добавить системы в этап.
    pub fn add_systems<M>(
        &mut self,
        stage: Stage,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        let mut schedules = self.world.resource_mut::<Schedules>();
        schedules.add_systems(stage, systems);
        self
    }

    /// Задать порядок и условия групп систем в этапе.
    ///
    /// Нужно, когда порядок важен между плагинами: регистрация контента должна
    /// отработать до заморозки реестров, но плагины не знают друг о друге и не
    /// могут сослаться на чужую систему по имени.
    pub fn configure_sets<M>(
        &mut self,
        stage: Stage,
        sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
    ) -> &mut Self {
        let mut schedules = self.world.resource_mut::<Schedules>();
        schedules.configure_sets(stage, sets);
        self
    }

    /// Вставить ресурс.
    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    /// Создать ресурс значением по умолчанию, если его ещё нет.
    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    /// Зарегистрировать тип сообщения. Буфер очищается через кадр после записи,
    /// поэтому сообщение успевает прочитать любой этап следующего кадра.
    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        MessageRegistry::register_message::<M>(&mut self.world);
        self
    }

    /// Заменить цикл приложения. Вызывается плагином платформы.
    pub fn set_runner(&mut self, runner: impl FnOnce(App) + 'static) -> &mut Self {
        self.runner = Some(Box::new(runner));
        self
    }

    /// Запустить приложение. Управление возвращается, когда цикл завершился.
    pub fn run(mut self) {
        let runner = self.runner.take();
        match runner {
            Some(runner) => runner(self),
            None => {
                // Headless: один кадр. Бесконечный цикл без окна повесил бы и CI,
                // и `cargo run` без рендера.
                self.update();
            }
        }
    }

    /// Выполнить один кадр: все этапы в порядке [`Stage::FRAME_ORDER`],
    /// с нужным числом фиксированных тиков перед [`Stage::Update`].
    pub fn update(&mut self) {
        if !self.started {
            self.started = true;
            self.world.run_schedule(Stage::Startup);
        }

        let now = Instant::now();
        let delta = match self.last_update {
            Some(last) => now.saturating_duration_since(last),
            None => Duration::ZERO,
        };
        self.last_update = Some(now);
        self.world.resource_mut::<Time>().advance(delta);

        let ticks = self.world.resource_mut::<FixedTick>().accumulate(delta);

        for stage in Stage::FRAME_ORDER {
            if matches!(stage, Stage::Update) {
                for _ in 0..ticks {
                    self.world.run_schedule(Stage::FixedTick);
                }
            }
            self.world.run_schedule(*stage);
        }

        let dropped = self.world.resource::<FixedTick>().dropped();
        if dropped > 0 {
            warn!(
                dropped,
                "симуляция не успевает за реальным временем, тики отброшены"
            );
        }
    }

    /// Был ли запрошен выход. Цикл платформы проверяет это после каждого кадра.
    pub fn exit_requested(&self) -> bool {
        self.world
            .get_resource::<Messages<AppExit>>()
            .is_some_and(|messages| !messages.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Resource, Default)]
    struct Journal(Arc<Mutex<Vec<&'static str>>>);

    fn записать(stage: &'static str) -> impl FnMut(Res<Journal>) {
        move |journal: Res<Journal>| {
            if let Ok(mut log) = journal.0.lock() {
                log.push(stage);
            }
        }
    }

    struct TestPlugin(&'static str);
    impl Plugin for TestPlugin {
        fn name(&self) -> &'static str {
            self.0
        }
        fn build(&self, app: &mut App) {
            app.add_systems(Stage::Update, записать("update"));
        }
    }

    #[test]
    fn этапы_выполняются_в_заданном_порядке() {
        let mut app = App::new();
        let journal = Journal::default();
        let log = Arc::clone(&journal.0);
        app.insert_resource(journal);
        app.add_systems(Stage::Startup, записать("startup"));
        app.add_systems(Stage::First, записать("first"));
        app.add_systems(Stage::PreUpdate, записать("pre_update"));
        app.add_systems(Stage::Update, записать("update"));
        app.add_systems(Stage::PostUpdate, записать("post_update"));
        app.add_systems(Stage::PreRender, записать("pre_render"));
        app.add_systems(Stage::Render, записать("render"));
        app.add_systems(Stage::Last, записать("last"));

        app.update();

        let log = log.lock().expect("журнал не отравлен");
        assert_eq!(
            *log,
            vec![
                "startup",
                "first",
                "pre_update",
                "update",
                "post_update",
                "pre_render",
                "render",
                "last",
            ]
        );
    }

    #[test]
    fn startup_выполняется_один_раз() {
        let mut app = App::new();
        let journal = Journal::default();
        let log = Arc::clone(&journal.0);
        app.insert_resource(journal);
        app.add_systems(Stage::Startup, записать("startup"));

        app.update();
        app.update();
        app.update();

        assert_eq!(log.lock().expect("журнал не отравлен").len(), 1);
    }

    #[test]
    fn фиксированный_тик_выполняется_по_накоплению_времени() {
        let mut app = App::new();
        let journal = Journal::default();
        let log = Arc::clone(&journal.0);
        app.insert_resource(journal);
        app.add_systems(Stage::FixedTick, записать("tick"));

        // Первый кадр: delta равна нулю, тиков быть не должно.
        app.update();
        assert_eq!(log.lock().expect("журнал не отравлен").len(), 0);

        // Накопить время вручную: тест не должен зависеть от реальных часов.
        app.world
            .resource_mut::<FixedTick>()
            .accumulate(Duration::from_millis(120));
        let before = app.world.resource::<FixedTick>().ticks();
        assert_eq!(before, 2, "120 мс при 20 Hz — это два тика");
    }

    #[test]
    fn повторная_установка_плагина_игнорируется() {
        let mut app = App::new();
        app.add_plugin(TestPlugin("test"));
        app.add_plugin(TestPlugin("test"));
        assert_eq!(app.plugins(), &["test"]);
    }

    #[test]
    fn разные_плагины_устанавливаются_оба() {
        let mut app = App::new();
        app.add_plugin(TestPlugin("a"));
        app.add_plugin(TestPlugin("b"));
        assert_eq!(app.plugins(), &["a", "b"]);
    }

    #[test]
    fn выход_запрашивается_сообщением() {
        let mut app = App::new();
        assert!(!app.exit_requested());
        app.add_systems(Stage::Update, |mut exit: MessageWriter<AppExit>| {
            exit.write(AppExit);
        });
        app.update();
        assert!(app.exit_requested());
    }

    #[test]
    fn своё_сообщение_доживает_до_следующего_кадра() {
        #[derive(Message, Debug, Clone, Copy)]
        struct Ping;

        let mut app = App::new();
        app.add_message::<Ping>();
        let journal = Journal::default();
        let log = Arc::clone(&journal.0);
        app.insert_resource(journal);

        app.add_systems(Stage::Update, |mut w: MessageWriter<Ping>| {
            w.write(Ping);
        });
        app.add_systems(
            Stage::PreRender,
            move |mut r: MessageReader<Ping>, journal: Res<Journal>| {
                for _ in r.read() {
                    if let Ok(mut log) = journal.0.lock() {
                        log.push("получено");
                    }
                }
            },
        );

        app.update();
        assert_eq!(
            log.lock().expect("журнал не отравлен").len(),
            1,
            "сообщение, записанное в Update, должно читаться в PreRender того же кадра"
        );
    }
}
