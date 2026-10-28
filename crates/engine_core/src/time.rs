//! Время кадра и фиксированный тик симуляции.

use bevy_ecs::resource::Resource;
use core::time::Duration;

/// Время кадра. Обновляется в начале каждого кадра.
#[derive(Resource, Debug, Clone, Default)]
pub struct Time {
    delta: Duration,
    elapsed: Duration,
    frame: u64,
}

impl Time {
    /// Длительность прошлого кадра.
    #[inline]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    /// Длительность прошлого кадра в секундах — для интеграции движения.
    #[inline]
    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Время с запуска приложения.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Номер текущего кадра, начиная с нуля.
    #[inline]
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Продвинуть время на длительность кадра. Вызывает [`crate::App`].
    pub(crate) fn advance(&mut self, delta: Duration) {
        self.delta = delta;
        self.elapsed += delta;
        self.frame += 1;
    }
}

/// Аккумулятор фиксированного тика симуляции.
///
/// Логика мира должна идти с постоянным шагом, иначе симуляция зависит от FPS.
/// Рендер при этом остаётся переменным: интерполировать между тиками позволяет
/// [`FixedTick::fraction`].
#[derive(Resource, Debug, Clone)]
pub struct FixedTick {
    period: Duration,
    accumulator: Duration,
    max_catch_up: u32,
    ticks: u64,
    fraction: f32,
    dropped: u64,
}

impl Default for FixedTick {
    fn default() -> Self {
        Self::from_hz(20.0)
    }
}

impl FixedTick {
    /// Тик с заданной частотой в герцах.
    ///
    /// # Паника
    ///
    /// Если `hz` не положителен или не конечен.
    pub fn from_hz(hz: f64) -> Self {
        assert!(
            hz.is_finite() && hz > 0.0,
            "частота фиксированного тика должна быть положительной и конечной, получено {hz}"
        );
        Self {
            period: Duration::from_secs_f64(1.0 / hz),
            accumulator: Duration::ZERO,
            // При просадке кадра накопленные тики выполняются подряд, но не бесконечно:
            // без лимита тяжёлый кадр вызывает ещё больше тиков и спираль замедления.
            max_catch_up: 5,
            ticks: 0,
            fraction: 0.0,
            dropped: 0,
        }
    }

    /// Шаг одного тика.
    #[inline]
    pub fn period(&self) -> Duration {
        self.period
    }

    /// Шаг одного тика в секундах — постоянный, пригоден для интеграции.
    #[inline]
    pub fn period_secs(&self) -> f32 {
        self.period.as_secs_f32()
    }

    /// Сколько тиков выполнено с запуска.
    #[inline]
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Доля накопленного времени до следующего тика, `0.0..1.0`.
    /// Рендер интерполирует состояние по этому значению.
    #[inline]
    pub fn fraction(&self) -> f32 {
        self.fraction
    }

    /// Сколько тиков отброшено из-за лимита догона. Больше нуля — симуляция
    /// не успевает за реальным временем.
    #[inline]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Максимальное число тиков за один кадр.
    #[inline]
    pub fn max_catch_up(&self) -> u32 {
        self.max_catch_up
    }

    /// Накопить время кадра и вернуть, сколько тиков нужно выполнить.
    pub(crate) fn accumulate(&mut self, delta: Duration) -> u32 {
        self.accumulator += delta;
        let mut runs = 0;
        while self.accumulator >= self.period && runs < self.max_catch_up {
            self.accumulator -= self.period;
            runs += 1;
        }
        if self.accumulator >= self.period {
            // Догнать не успеваем: отбрасываем остаток, иначе долг растёт без предела.
            let behind = self.accumulator.as_nanos() / self.period.as_nanos().max(1);
            self.dropped += behind as u64;
            self.accumulator = Duration::ZERO;
        }
        self.ticks += u64::from(runs);
        self.fraction = self.accumulator.as_secs_f32() / self.period.as_secs_f32();
        runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn тик_не_выполняется_пока_не_накоплен_период() {
        let mut tick = FixedTick::from_hz(20.0); // 50 мс
        assert_eq!(tick.accumulate(Duration::from_millis(20)), 0);
        assert_eq!(tick.accumulate(Duration::from_millis(20)), 0);
        assert_eq!(tick.accumulate(Duration::from_millis(20)), 1);
        assert_eq!(tick.ticks(), 1);
    }

    #[test]
    fn накопленное_время_догоняется_несколькими_тиками() {
        let mut tick = FixedTick::from_hz(20.0);
        assert_eq!(tick.accumulate(Duration::from_millis(160)), 3);
        assert_eq!(tick.ticks(), 3);
        assert_eq!(tick.dropped(), 0);
    }

    #[test]
    fn догон_ограничен_и_остаток_отбрасывается() {
        let mut tick = FixedTick::from_hz(20.0);
        // Секунда простоя — это 20 тиков, но лимит догона 5.
        let runs = tick.accumulate(Duration::from_secs(1));
        assert_eq!(runs, tick.max_catch_up());
        assert!(tick.dropped() > 0, "остаток должен быть отброшен");
        // Долг не копится: следующий обычный кадр снова даёт ноль или один тик.
        assert!(tick.accumulate(Duration::from_millis(16)) <= 1);
    }

    #[test]
    fn доля_кадра_растёт_между_тиками() {
        let mut tick = FixedTick::from_hz(20.0);
        tick.accumulate(Duration::from_millis(25));
        let a = tick.fraction();
        tick.accumulate(Duration::from_millis(10));
        assert!(
            tick.fraction() > a,
            "{} должно быть больше {a}",
            tick.fraction()
        );
        assert!(tick.fraction() < 1.0);
    }

    #[test]
    fn время_кадра_накапливается() {
        let mut time = Time::default();
        time.advance(Duration::from_millis(16));
        time.advance(Duration::from_millis(16));
        assert_eq!(time.frame(), 2);
        assert_eq!(time.elapsed(), Duration::from_millis(32));
        assert_eq!(time.delta(), Duration::from_millis(16));
    }

    #[test]
    #[should_panic(expected = "положительной")]
    fn нулевая_частота_тика_запрещена() {
        let _ = FixedTick::from_hz(0.0);
    }
}
