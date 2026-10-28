//! Шум значений с интерполяцией и фрактальным суммированием.
//!
//! Своя реализация вместо крейта: генерация обязана быть детерминированной между
//! запусками и версиями — иначе выгруженный и заново загруженный чанк не совпадёт
//! с соседями, и на стыке появится шов. Чужая реализация такого обещания не даёт:
//! обновление крейта может поменять значения, и все существующие миры поедут.
//!
//! Это шум значений (value noise), а не градиентный: он чуть более «клетчатый»,
//! зато вдвое дешевле, а для холмов в несколько октав разница не видна.

/// Перемешать целое. Основа детерминизма: от этой функции зависит весь рельеф.
#[inline]
const fn mix(mut value: u64) -> u64 {
    // splitmix64: дёшево и хорошо перемешивает даже близкие значения,
    // а именно близкие нам и приходят — соседние координаты.
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Хеш пары целых координат с солью.
#[inline]
pub const fn hash2(seed: u64, x: i32, z: i32) -> u64 {
    // Координаты раскладываются в разные половины, чтобы (1, 0) и (0, 1)
    // не давали один результат.
    let packed = ((x as i64 as u64) << 32) ^ (z as i64 as u64 & 0xffff_ffff);
    mix(seed ^ mix(packed))
}

/// Случайное число `0.0..1.0` по целым координатам.
#[inline]
pub fn value_at(seed: u64, x: i32, z: i32) -> f32 {
    // Берутся старшие биты: младшие у любого смешивания хуже распределены.
    (hash2(seed, x, z) >> 40) as f32 / (1u64 << 24) as f32
}

/// Сглаживающая кривая: производная равна нулю на концах, поэтому на границах
/// ячеек решётки нет изломов.
#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Шум значений в точке. Возвращает `0.0..1.0`.
pub fn noise2(seed: u64, x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let tx = smoothstep(x - xi);
    let tz = smoothstep(z - zi);

    // Приведение к i32 насыщающее: на краю диапазона мир не должен заворачиваться
    // в другую точку и давать видимый шов.
    let xi = xi.clamp(i32::MIN as f32, i32::MAX as f32) as i32;
    let zi = zi.clamp(i32::MIN as f32, i32::MAX as f32) as i32;

    let v00 = value_at(seed, xi, zi);
    let v10 = value_at(seed, xi.wrapping_add(1), zi);
    let v01 = value_at(seed, xi, zi.wrapping_add(1));
    let v11 = value_at(seed, xi.wrapping_add(1), zi.wrapping_add(1));

    let bottom = v00 + (v10 - v00) * tx;
    let top = v01 + (v11 - v01) * tx;
    bottom + (top - bottom) * tz
}

/// Параметры фрактального шума.
#[derive(Debug, Clone, Copy)]
pub struct Fbm {
    /// Сколько слоёв суммируется.
    pub octaves: u32,
    /// Во сколько раз растёт частота следующего слоя.
    pub lacunarity: f32,
    /// Во сколько раз падает вклад следующего слоя.
    pub gain: f32,
    /// Частота первого слоя: сколько ячеек решётки на блок.
    pub frequency: f32,
}

impl Default for Fbm {
    fn default() -> Self {
        Self {
            octaves: 4,
            lacunarity: 2.0,
            gain: 0.5,
            frequency: 1.0 / 128.0,
        }
    }
}

impl Fbm {
    /// Фрактальный шум в точке. Возвращает `0.0..1.0`.
    ///
    /// Сумма слоёв нормируется на суммарную амплитуду, иначе результат зависел бы
    /// от числа октав и подбирать высоты пришлось бы заново при каждой правке.
    pub fn sample(&self, seed: u64, x: f32, z: f32) -> f32 {
        let mut frequency = self.frequency;
        let mut amplitude = 1.0;
        let mut total = 0.0;
        let mut normalizer = 0.0;

        for octave in 0..self.octaves {
            // Своя соль у каждой октавы: иначе слои совпадут в точках решётки
            // и дадут регулярную сетку вместо шума.
            let octave_seed = seed ^ mix(u64::from(octave) + 1);
            total += noise2(octave_seed, x * frequency, z * frequency) * amplitude;
            normalizer += amplitude;
            frequency *= self.lacunarity;
            amplitude *= self.gain;
        }

        if normalizer > 0.0 {
            total / normalizer
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn шум_детерминирован() {
        for (x, z) in [(0.0, 0.0), (12.5, -7.25), (-100.75, 500.5)] {
            let a = noise2(42, x, z);
            let b = noise2(42, x, z);
            assert_eq!(a, b, "({x}, {z})");
        }
    }

    #[test]
    fn шум_в_диапазоне_от_нуля_до_единицы() {
        for i in -500..500 {
            let x = i as f32 * 0.37;
            let z = i as f32 * -0.61;
            let value = noise2(7, x, z);
            assert!((0.0..=1.0).contains(&value), "({x}, {z}) дало {value}");
        }
    }

    #[test]
    fn разные_seed_дают_разный_рельеф() {
        let a: Vec<f32> = (0..64).map(|i| noise2(1, i as f32 * 0.5, 0.0)).collect();
        let b: Vec<f32> = (0..64).map(|i| noise2(2, i as f32 * 0.5, 0.0)).collect();
        assert_ne!(a, b);
    }

    #[test]
    fn шум_непрерывен() {
        // Соседние точки не должны прыгать: иначе рельеф получит отвесные стены
        // на границах ячеек решётки.
        let step = 0.05;
        let mut previous = noise2(3, 0.0, 0.0);
        for i in 1..400 {
            let value = noise2(3, i as f32 * step, 0.0);
            assert!(
                (value - previous).abs() < 0.2,
                "скачок {previous} -> {value} на шаге {i}"
            );
            previous = value;
        }
    }

    #[test]
    fn значения_в_узлах_решётки_совпадают_с_табличными() {
        // В целых точках интерполяция должна давать ровно значение узла.
        for (x, z) in [(0, 0), (5, -3), (-17, 42)] {
            let direct = value_at(9, x, z);
            let interpolated = noise2(9, x as f32, z as f32);
            assert!(
                (direct - interpolated).abs() < 1e-6,
                "узел ({x}, {z}): {direct} против {interpolated}"
            );
        }
    }

    #[test]
    fn соседние_координаты_не_коллизируют() {
        // (1, 0) и (0, 1) не должны давать один хеш.
        assert_ne!(hash2(0, 1, 0), hash2(0, 0, 1));
        assert_ne!(hash2(0, -1, 0), hash2(0, 0, -1));
        assert_ne!(hash2(0, 0, 0), hash2(1, 0, 0));
    }

    #[test]
    fn фрактальный_шум_в_диапазоне() {
        let fbm = Fbm::default();
        for i in -300..300 {
            let x = i as f32 * 1.7;
            let z = i as f32 * -2.3;
            let value = fbm.sample(11, x, z);
            assert!((0.0..=1.0).contains(&value), "({x}, {z}) дало {value}");
        }
    }

    #[test]
    fn фрактальный_шум_использует_все_октавы() {
        // Одна октава и четыре должны давать разный результат, иначе соль
        // октав не работает и слои складываются в одно и то же.
        let one = Fbm {
            octaves: 1,
            ..Fbm::default()
        };
        let four = Fbm::default();
        let differences = (0..100)
            .filter(|i| {
                let x = *i as f32 * 3.0;
                (one.sample(5, x, 0.0) - four.sample(5, x, 0.0)).abs() > 1e-4
            })
            .count();
        assert!(
            differences > 90,
            "октавы почти не влияют: {differences}/100"
        );
    }

    #[test]
    fn фрактальный_шум_покрывает_широкий_диапазон() {
        // Если нормировка сломана, значения собьются к середине и рельеф
        // станет плоским.
        let fbm = Fbm::default();
        let values: Vec<f32> = (0..2000)
            .map(|i| fbm.sample(13, i as f32 * 2.5, (i % 97) as f32 * 3.5))
            .collect();
        let min = values.iter().copied().fold(f32::INFINITY, f32::min);
        let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(max - min > 0.4, "диапазон всего {min}..{max}");
    }
}
