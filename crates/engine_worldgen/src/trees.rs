//! Размещение деревьев.
//!
//! Дерево шире одного блока, поэтому почти всегда пересекает границу чанка.
//! Наивный подход — «сгенерировать деревья своего чанка» — обрезал бы крону там,
//! где она выходит за край.
//!
//! Решение: колонка смотрит не только на свои деревья, но и на деревья соседних
//! чанков в радиусе [`TREE_CHUNK_RADIUS`], и записывает те их блоки, что попали
//! в неё. Это работает, потому что набор деревьев чанка зависит **только** от
//! seed, координат чанка и карты высот — ни от порядка генерации, ни от того,
//! загружены ли соседи. Любая колонка приходит к одному и тому же выводу о том,
//! где стоит дерево и какие блоки оно занимает.

use crate::noise::hash2;
use crate::palette::TerrainPalette;
use crate::terrain::{HeightMap, SEA_LEVEL};
use engine_world::{Column, MAX_Y};

/// В скольких чанках вокруг искать деревья, которые могут дотянуться до нашего.
///
/// Крона не шире [`LEAVES_RADIUS`], так что одного чанка хватает с запасом.
const TREE_CHUNK_RADIUS: i32 = 1;

/// Сколько попыток посадить дерево делается на чанк.
const TREE_ATTEMPTS: u32 = 6;

/// Какая доля попыток приживается. Чем меньше, тем реже лес.
const TREE_CHANCE: f32 = 0.22;

/// Минимальная высота ствола.
const TRUNK_MIN: i32 = 4;
/// Максимальная высота ствола.
const TRUNK_MAX: i32 = 7;
/// Радиус кроны по горизонтали.
const LEAVES_RADIUS: i32 = 2;

/// Дерево: ствол и крона, заданные абсолютными координатами.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tree {
    /// Мировая X ствола.
    x: i32,
    /// Мировая Z ствола.
    z: i32,
    /// Y нижнего блока ствола (на один выше поверхности).
    base_y: i32,
    /// Высота ствола в блоках.
    trunk: i32,
}

/// Деревья, посаженные в одном чанке.
///
/// Зависит только от seed, координат чанка и карты высот — поэтому любой сосед
/// получает тот же результат.
fn trees_in_chunk(seed: u64, heights: &HeightMap, chunk_x: i32, chunk_z: i32) -> Vec<Tree> {
    let mut trees = Vec::new();

    for attempt in 0..TREE_ATTEMPTS {
        // Своя соль у попытки: иначе все попытки дали бы одну точку.
        let hash = hash2(seed ^ 0x7ee5 ^ u64::from(attempt) << 32, chunk_x, chunk_z);

        let roll = (hash >> 40) as f32 / (1u64 << 24) as f32;
        if roll > TREE_CHANCE {
            continue;
        }

        let local_x = ((hash >> 8) & 15) as i32;
        let local_z = ((hash >> 12) & 15) as i32;
        let x = chunk_x * 16 + local_x;
        let z = chunk_z * 16 + local_z;

        let surface = heights.height(x, z);
        // На песке и под водой деревья не растут: пляж должен оставаться пляжем.
        if surface <= SEA_LEVEL + 2 {
            continue;
        }

        let trunk = TRUNK_MIN + ((hash >> 16) % (TRUNK_MAX - TRUNK_MIN + 1) as u64) as i32;
        let base_y = surface + 1;
        if base_y + trunk + LEAVES_RADIUS >= MAX_Y {
            continue;
        }

        // Два дерева в одной точке — это просто одно дерево; но если две попытки
        // дали близкие точки, лес становится неестественно густым.
        if trees
            .iter()
            .any(|other: &Tree| (other.x - x).abs() < 4 && (other.z - z).abs() < 4)
        {
            continue;
        }

        trees.push(Tree {
            x,
            z,
            base_y,
            trunk,
        });
    }

    trees
}

/// Записать в колонку блоки дерева, попавшие в неё.
fn place_tree(tree: &Tree, column: &mut Column, palette: &TerrainPalette) {
    let origin = column.position().origin(0);
    let top = tree.base_y + tree.trunk - 1;

    // Крона: слои вокруг верхушки ствола.
    for dy in -1..=2 {
        let y = top + dy;
        // Верхний слой у́же: иначе крона выглядит кубом.
        let radius = match dy {
            2 => LEAVES_RADIUS - 1,
            _ => LEAVES_RADIUS,
        };
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                // Углы самого широкого слоя срезаны — крона становится округлой.
                if dx.abs() == radius && dz.abs() == radius && dy < 2 {
                    continue;
                }
                // Ствол не затирается листвой.
                if dx == 0 && dz == 0 && y <= top {
                    continue;
                }
                write_block(column, origin, tree.x + dx, y, tree.z + dz, palette.leaves);
            }
        }
    }

    // Ствол пишется после кроны: он важнее и должен быть виден целиком.
    for y in tree.base_y..=top {
        write_block(column, origin, tree.x, y, tree.z, palette.log);
    }
}

/// Записать блок, если он попадает в эту колонку.
#[inline]
fn write_block(
    column: &mut Column,
    origin: engine_world::BlockPos,
    x: i32,
    y: i32,
    z: i32,
    block: engine_world::BlockId,
) {
    let local_x = x - origin.x;
    let local_z = z - origin.z;
    if !(0..16).contains(&local_x) || !(0..16).contains(&local_z) {
        return;
    }
    column.set(local_x as u8, y, local_z as u8, block);
}

/// Посадить в колонку все деревья, которые до неё дотягиваются.
pub fn place_trees(seed: u64, heights: &HeightMap, column: &mut Column, palette: &TerrainPalette) {
    let position = column.position();
    for chunk_z in (position.z - TREE_CHUNK_RADIUS)..=(position.z + TREE_CHUNK_RADIUS) {
        for chunk_x in (position.x - TREE_CHUNK_RADIUS)..=(position.x + TREE_CHUNK_RADIUS) {
            for tree in trees_in_chunk(seed, heights, chunk_x, chunk_z) {
                place_tree(&tree, column, palette);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::tests::тестовая_палитра;
    use crate::terrain::fill_terrain;
    use engine_world::{BlockId, ChunkPos};

    fn сгенерировать(
        seed: u64,
        position: ChunkPos,
        palette: &TerrainPalette,
    ) -> Column {
        let heights = HeightMap::new(seed);
        let mut column = Column::new(position);
        fill_terrain(&heights, &mut column, palette);
        place_trees(seed, &heights, &mut column, palette);
        column
    }

    /// Найти чанк, в котором выросло хотя бы одно дерево.
    fn чанк_с_деревом(seed: u64) -> Option<ChunkPos> {
        let heights = HeightMap::new(seed);
        (-15..15)
            .flat_map(|cx| (-15..15).map(move |cz| (cx, cz)))
            .find(|(cx, cz)| !trees_in_chunk(seed, &heights, *cx, *cz).is_empty())
            .map(|(cx, cz)| ChunkPos::new(cx, cz))
    }

    #[test]
    fn деревья_вырастают() {
        assert!(
            чанк_с_деревом(2024).is_some(),
            "в радиусе 15 чанков не выросло ни одного дерева"
        );
    }

    #[test]
    fn набор_деревьев_детерминирован() {
        let heights = HeightMap::new(5);
        for (cx, cz) in [(0, 0), (3, -7), (-12, 9)] {
            let a = trees_in_chunk(5, &heights, cx, cz);
            let b = trees_in_chunk(5, &heights, cx, cz);
            assert_eq!(a, b, "чанк ({cx}, {cz})");
        }
    }

    #[test]
    fn дерево_не_обрезается_на_границе_чанка() {
        // Главное свойство: соседние колонки должны одинаково видеть одно дерево.
        // Берём дерево у самого края чанка и проверяем, что его блоки есть
        // и в своей колонке, и в соседней.
        let seed = 31337;
        let palette = тестовая_палитра();
        let heights = HeightMap::new(seed);

        // Ищем дерево, крона которого заведомо пересекает границу по X.
        let mut found = None;
        'outer: for cx in -15..15 {
            for cz in -15..15 {
                for tree in trees_in_chunk(seed, &heights, cx, cz) {
                    let local_x = tree.x - cx * 16;
                    if local_x >= 16 - LEAVES_RADIUS {
                        found = Some((ChunkPos::new(cx, cz), tree));
                        break 'outer;
                    }
                }
            }
        }
        let (position, tree) = found.expect("должно найтись дерево у границы чанка");

        let own = сгенерировать(seed, position, &palette);
        let neighbour = сгенерировать(seed, position.offset(1, 0), &palette);

        let top = tree.base_y + tree.trunk - 1;
        let mut листва_своя = 0;
        let mut листва_соседа = 0;
        for dz in -LEAVES_RADIUS..=LEAVES_RADIUS {
            for dx in -LEAVES_RADIUS..=LEAVES_RADIUS {
                let x = tree.x + dx;
                let z = tree.z + dz;
                let block = block_at(&own, &neighbour, position, x, top, z);
                if block == palette.leaves {
                    if x < (position.x + 1) * 16 {
                        листва_своя += 1;
                    } else {
                        листва_соседа += 1;
                    }
                }
            }
        }

        assert!(листва_своя > 0, "в своей колонке листвы нет");
        assert!(
            листва_соседа > 0,
            "крона обрезана на границе: сосед не знает о дереве"
        );
    }

    /// Блок по мировым координатам из двух соседних колонок.
    fn block_at(
        own: &Column,
        neighbour: &Column,
        position: ChunkPos,
        x: i32,
        y: i32,
        z: i32,
    ) -> BlockId {
        let origin = position.origin(0);
        let local_x = x - origin.x;
        let local_z = z - origin.z;
        if (0..16).contains(&local_x) && (0..16).contains(&local_z) {
            own.get(local_x as u8, y, local_z as u8)
        } else if (16..32).contains(&local_x) && (0..16).contains(&local_z) {
            neighbour.get((local_x - 16) as u8, y, local_z as u8)
        } else {
            BlockId::AIR
        }
    }

    #[test]
    fn у_дерева_есть_ствол_и_крона() {
        let seed = 2024;
        let palette = тестовая_палитра();
        let heights = HeightMap::new(seed);
        let position = чанк_с_деревом(seed).expect("дерево найдено");
        let trees = trees_in_chunk(seed, &heights, position.x, position.z);
        let tree = trees.first().copied().expect("дерево есть");

        let column = сгенерировать(seed, position, &palette);
        let origin = position.origin(0);
        let local_x = (tree.x - origin.x) as u8;
        let local_z = (tree.z - origin.z) as u8;

        for y in tree.base_y..(tree.base_y + tree.trunk) {
            assert_eq!(
                column.get(local_x, y, local_z),
                palette.log,
                "ствол на высоте {y}"
            );
        }
        let above = tree.base_y + tree.trunk;
        assert_eq!(
            column.get(local_x, above, local_z),
            palette.leaves,
            "над стволом должна быть листва"
        );
    }

    #[test]
    fn дерево_стоит_на_земле_а_не_в_воздухе() {
        let seed = 2024;
        let palette = тестовая_палитра();
        let heights = HeightMap::new(seed);
        let position = чанк_с_деревом(seed).expect("дерево найдено");
        let column = сгенерировать(seed, position, &palette);
        let origin = position.origin(0);

        for tree in trees_in_chunk(seed, &heights, position.x, position.z) {
            let local_x = (tree.x - origin.x) as u8;
            let local_z = (tree.z - origin.z) as u8;
            let under = column.get(local_x, tree.base_y - 1, local_z);
            assert!(!under.is_air(), "под стволом пустота");
            assert_ne!(under, palette.water, "дерево не должно расти из воды");
        }
    }

    #[test]
    fn деревья_не_растут_на_пляже() {
        let seed = 999;
        let heights = HeightMap::new(seed);
        for cx in -15..15 {
            for cz in -15..15 {
                for tree in trees_in_chunk(seed, &heights, cx, cz) {
                    let surface = heights.height(tree.x, tree.z);
                    assert!(
                        surface > SEA_LEVEL + 2,
                        "дерево на отметке {surface} при уровне моря {SEA_LEVEL}"
                    );
                }
            }
        }
    }

    #[test]
    fn деревья_не_стоят_вплотную() {
        let seed = 4242;
        let heights = HeightMap::new(seed);
        for cx in -10..10 {
            for cz in -10..10 {
                let trees = trees_in_chunk(seed, &heights, cx, cz);
                for (i, a) in trees.iter().enumerate() {
                    for b in &trees[i + 1..] {
                        assert!(
                            (a.x - b.x).abs() >= 4 || (a.z - b.z).abs() >= 4,
                            "деревья слишком близко: {a:?} и {b:?}"
                        );
                    }
                }
            }
        }
    }
}
