//! Рельеф: высота поверхности и слои блоков.

use crate::noise::Fbm;
use crate::palette::TerrainPalette;
use engine_world::{Column, SECTION_SIZE, SECTIONS_PER_COLUMN, Section};

/// Уровень моря. Ниже него впадины заливаются водой.
pub const SEA_LEVEL: i32 = 62;

/// Средняя высота поверхности.
pub const BASE_HEIGHT: i32 = 64;

/// Максимальное отклонение от средней высоты.
pub const AMPLITUDE: i32 = 40;

/// Толщина слоя земли под поверхностью.
const DIRT_DEPTH: i32 = 4;

/// Насколько выше уровня моря ещё лежит песок.
const BEACH_HEIGHT: i32 = 2;

/// Высота поверхности по мировым координатам.
///
/// Складывается из двух слоёв: низкочастотный задаёт крупные формы (равнины и
/// нагорья), высокочастотный добавляет холмы. Один слой дал бы либо равномерную
/// рябь, либо одинаково гладкие волны без разнообразия.
#[derive(Debug, Clone, Copy)]
pub struct HeightMap {
    seed: u64,
    continents: Fbm,
    hills: Fbm,
}

impl HeightMap {
    /// Карта высот для seed.
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            continents: Fbm {
                octaves: 3,
                frequency: 1.0 / 512.0,
                ..Fbm::default()
            },
            hills: Fbm {
                octaves: 4,
                frequency: 1.0 / 96.0,
                ..Fbm::default()
            },
        }
    }

    /// Высота поверхности в блоках: Y верхнего непустого блока.
    pub fn height(&self, x: i32, z: i32) -> i32 {
        let fx = x as f32;
        let fz = z as f32;

        // Континенты задают, насколько местность вообще приподнята.
        let continent = self.continents.sample(self.seed, fx, fz) * 2.0 - 1.0;
        // Холмы тем заметнее, чем выше местность: на равнинах у моря ровнее.
        let hill = self.hills.sample(self.seed ^ 0xa5a5, fx, fz) * 2.0 - 1.0;
        let hill_weight = 0.35 + 0.65 * continent.max(0.0);

        let offset = continent * 0.7 + hill * hill_weight * 0.5;
        BASE_HEIGHT + (offset * AMPLITUDE as f32).round() as i32
    }
}

/// Заполнить колонку рельефом: камень, земля, поверхность и вода.
///
/// Высоты считаются один раз на все 256 столбов, а секции, целиком лежащие ниже
/// самой низкой поверхности, заполняются одним значением вместо 4096 записей:
/// в колонке таких секций обычно больше половины.
pub fn fill_terrain(heights: &HeightMap, column: &mut Column, palette: &TerrainPalette) {
    let origin = column.position().origin(0);

    let mut surfaces = [0i32; 256];
    let mut lowest = i32::MAX;
    for local_z in 0..16usize {
        for local_x in 0..16usize {
            let surface = heights.height(origin.x + local_x as i32, origin.z + local_z as i32);
            surfaces[local_z * 16 + local_x] = surface;
            lowest = lowest.min(surface);
        }
    }

    // Ниже этой отметки вся колонка — сплошной камень.
    let solid_stone_top = lowest - DIRT_DEPTH;
    let mut first_mixed_section = 0;
    for index in 0..SECTIONS_PER_COLUMN {
        let base = Column::section_base_y(index);
        if base + SECTION_SIZE - 1 > solid_stone_top {
            break;
        }
        if let Some(section) = column.section_mut(index) {
            *section = Section::uniform(palette.stone);
        }
        first_mixed_section = index + 1;
    }
    let fill_from = Column::section_base_y(first_mixed_section);

    for local_z in 0..16u8 {
        for local_x in 0..16u8 {
            let surface = surfaces[usize::from(local_z) * 16 + usize::from(local_x)];

            let underwater = surface <= SEA_LEVEL;
            let beach = surface <= SEA_LEVEL + BEACH_HEIGHT;
            // Поверхность у воды — песок: пляжи и дно.
            let top_block = if underwater || beach {
                palette.sand
            } else {
                palette.grass
            };
            let filler_block = if underwater || beach {
                palette.sand
            } else {
                palette.dirt
            };

            for y in fill_from..=surface {
                let block = if y == surface {
                    top_block
                } else if y > surface - DIRT_DEPTH {
                    filler_block
                } else {
                    palette.stone
                };
                column.set(local_x, y, local_z, block);
            }

            for y in (surface + 1)..=SEA_LEVEL {
                column.set(local_x, y, local_z, palette.water);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::tests::тестовая_палитра;
    use engine_world::ChunkPos;
    use engine_world::MIN_Y;

    #[test]
    fn высота_детерминирована() {
        let heights = HeightMap::new(123);
        for (x, z) in [(0, 0), (37, -91), (-1000, 2000)] {
            assert_eq!(heights.height(x, z), heights.height(x, z), "({x}, {z})");
        }
    }

    #[test]
    fn высота_в_заявленном_диапазоне() {
        let heights = HeightMap::new(7);
        for x in -600..600 {
            for z in [-500, -17, 0, 123] {
                let h = heights.height(x, z);
                assert!(
                    (BASE_HEIGHT - AMPLITUDE..=BASE_HEIGHT + AMPLITUDE).contains(&h),
                    "({x}, {z}) дало {h}"
                );
            }
        }
    }

    #[test]
    fn рельеф_непрерывен() {
        // Соседние блоки не должны отличаться на отвесную стену.
        let heights = HeightMap::new(55);
        for x in -300..300 {
            let a = heights.height(x, 0);
            let b = heights.height(x + 1, 0);
            assert!((a - b).abs() <= 3, "обрыв {a} -> {b} при x={x}");
        }
    }

    #[test]
    fn разные_seed_дают_разный_рельеф() {
        let a = HeightMap::new(1);
        let b = HeightMap::new(2);
        let differences = (0..200)
            .filter(|x| a.height(*x, 0) != b.height(*x, 0))
            .count();
        assert!(
            differences > 150,
            "рельефы слишком похожи: {differences}/200"
        );
    }

    #[test]
    fn рельеф_не_плоский() {
        let heights = HeightMap::new(99);
        let values: Vec<i32> = (0..2000).map(|i| heights.height(i * 3, i % 211)).collect();
        let min = values.iter().copied().min().unwrap_or(0);
        let max = values.iter().copied().max().unwrap_or(0);
        assert!(max - min > 30, "перепад высот всего {min}..{max}");
    }

    #[test]
    fn колонка_заполняется_слоями() {
        let palette = тестовая_палитра();
        let heights = HeightMap::new(42);
        let mut column = Column::new(ChunkPos::new(0, 0));
        fill_terrain(&heights, &mut column, &palette);

        for local_x in [0u8, 7, 15] {
            for local_z in [0u8, 3, 15] {
                let surface = heights.height(i32::from(local_x), i32::from(local_z));

                // Под поверхностью — камень, прямо под ней — наполнитель.
                assert_eq!(
                    column.get(local_x, surface - DIRT_DEPTH, local_z),
                    palette.stone,
                    "глубина должна быть камнем"
                );
                assert_ne!(
                    column.get(local_x, surface, local_z),
                    palette.stone,
                    "поверхность не камень"
                );

                // Над водой и над поверхностью — воздух.
                let above = surface.max(SEA_LEVEL) + 1;
                assert!(
                    column.get(local_x, above, local_z).is_air(),
                    "над поверхностью должен быть воздух"
                );

                // Дыр в толще нет.
                for y in (surface - 20).max(MIN_Y)..=surface {
                    assert!(
                        !column.get(local_x, y, local_z).is_air(),
                        "дыра на ({local_x}, {y}, {local_z})"
                    );
                }
            }
        }
    }

    #[test]
    fn под_поверхностью_нет_пустот_до_самого_дна() {
        // Секции ниже самой низкой поверхности заполняются целиком, а не
        // поблочно. Ошибка на единицу в границе дала бы слой воздуха на стыке
        // с поблочной частью — провал, видимый только при полёте под землёй.
        let palette = тестовая_палитра();
        let heights = HeightMap::new(31337);

        for position in [
            ChunkPos::new(0, 0),
            ChunkPos::new(-3, 7),
            ChunkPos::new(12, -9),
        ] {
            let mut column = Column::new(position);
            fill_terrain(&heights, &mut column, &palette);
            let origin = position.origin(0);

            for local_x in 0..16u8 {
                for local_z in 0..16u8 {
                    let surface = heights
                        .height(origin.x + i32::from(local_x), origin.z + i32::from(local_z));
                    for y in MIN_Y..=surface {
                        assert!(
                            !column.get(local_x, y, local_z).is_air(),
                            "дыра в {position:?} на ({local_x}, {y}, {local_z}), поверхность {surface}"
                        );
                    }
                }
            }
        }
    }

    /// Найти колонку, в которой есть блоки ниже уровня моря.
    ///
    /// Перебор вместо подобранного seed: иначе тест ломается от любой правки
    /// параметров рельефа, хотя проверяемое свойство не менялось.
    fn колонка_с_водой(heights: &HeightMap) -> Option<ChunkPos> {
        (-20..20)
            .flat_map(|cx| (-20..20).map(move |cz| ChunkPos::new(cx, cz)))
            .find(|position| {
                let origin = position.origin(0);
                (0..16).any(|dx| {
                    (0..16).any(|dz| heights.height(origin.x + dx, origin.z + dz) < SEA_LEVEL)
                })
            })
    }

    #[test]
    fn впадины_заливаются_водой_до_уровня_моря() {
        let palette = тестовая_палитра();
        let heights = HeightMap::new(2024);
        let position = колонка_с_водой(&heights).expect("хоть где-то должна быть вода");

        let mut column = Column::new(position);
        fill_terrain(&heights, &mut column, &palette);
        let origin = position.origin(0);

        let mut нашли_воду = false;
        for local_x in 0..16u8 {
            for local_z in 0..16u8 {
                let surface =
                    heights.height(origin.x + i32::from(local_x), origin.z + i32::from(local_z));
                if surface >= SEA_LEVEL {
                    continue;
                }
                нашли_воду = true;
                assert_eq!(
                    column.get(local_x, surface + 1, local_z),
                    palette.water,
                    "над дном должна быть вода"
                );
                assert_eq!(
                    column.get(local_x, SEA_LEVEL, local_z),
                    palette.water,
                    "вода должна доходить до уровня моря"
                );
                assert!(
                    column.get(local_x, SEA_LEVEL + 1, local_z).is_air(),
                    "выше уровня моря воды быть не должно"
                );
            }
        }
        assert!(нашли_воду, "колонка выбрана по наличию воды");
    }

    #[test]
    fn вода_встречается_в_мире() {
        // Если уровень моря недостижим, половина правил рельефа мертва.
        let heights = HeightMap::new(2024);
        assert!(
            колонка_с_водой(&heights).is_some(),
            "в радиусе 20 чанков не нашлось ни одной впадины ниже уровня моря"
        );
    }

    #[test]
    fn у_воды_песок_а_выше_трава() {
        let palette = тестовая_палитра();
        let heights = HeightMap::new(2024);
        let mut column = Column::new(ChunkPos::new(3, -5));
        fill_terrain(&heights, &mut column, &palette);
        let origin = column.position().origin(0);

        for local_x in 0..16u8 {
            for local_z in 0..16u8 {
                let surface =
                    heights.height(origin.x + i32::from(local_x), origin.z + i32::from(local_z));
                let top = column.get(local_x, surface, local_z);
                if surface <= SEA_LEVEL + BEACH_HEIGHT {
                    assert_eq!(top, palette.sand, "у воды должен быть песок");
                } else {
                    assert_eq!(top, palette.grass, "выше пляжа должна быть трава");
                }
            }
        }
    }

    #[test]
    fn соседние_колонки_стыкуются_по_высоте() {
        // Генерация колонки не должна зависеть ни от чего, кроме координат:
        // иначе на стыке чанков появится шов.
        let palette = тестовая_палитра();
        let heights = HeightMap::new(777);

        let mut left = Column::new(ChunkPos::new(-1, 0));
        let mut right = Column::new(ChunkPos::new(0, 0));
        fill_terrain(&heights, &mut left, &palette);
        fill_terrain(&heights, &mut right, &palette);

        for local_z in 0..16u8 {
            // Последний столб левой колонки — это x = -1, первый правой — x = 0.
            let left_height = heights.height(-1, i32::from(local_z));
            let right_height = heights.height(0, i32::from(local_z));
            assert!(
                (left_height - right_height).abs() <= 3,
                "обрыв на границе чанков при z={local_z}"
            );
            assert!(!left.get(15, left_height, local_z).is_air());
            assert!(!right.get(0, right_height, local_z).is_air());
        }
    }
}
