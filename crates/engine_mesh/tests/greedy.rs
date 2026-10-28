//! Эталонные случаи мешинга.

use engine_mesh::{MeshData, PaddedSection, TextureLookup, greedy_mesh};
use engine_registry::Identifier;
use engine_world::{BlockId, BlockProperties, BlockRegistry, Blocks, Face, SECTION_SIZE};

/// Текстуры-заглушки: номер текстуры равен номеру блока, у верхней грани свой.
///
/// Разные номера у граней нужны, чтобы тесты ловили перепутанные направления.
struct Textures;

impl TextureLookup for Textures {
    fn texture_index(&self, block: BlockId, face: Face) -> u32 {
        u32::from(block.get()) * 8 + face.index() as u32
    }
}

fn реестр() -> Blocks {
    let mut registry = BlockRegistry::new();
    registry
        .register(
            Identifier::parse(engine_world::AIR_ID).expect("корректный id"),
            BlockProperties::AIR,
        )
        .expect("воздух регистрируется");
    for name in ["test:stone", "test:dirt", "test:glass"] {
        let properties = if name == "test:glass" {
            // Прозрачный, но не пустой: грани соседей рядом с ним рисуются.
            BlockProperties {
                opaque: false,
                light: 0,
                empty: false,
            }
        } else {
            BlockProperties::SOLID
        };
        registry
            .register(Identifier::parse(name).expect("корректный id"), properties)
            .expect("блок регистрируется");
    }
    registry.freeze().expect("заморозка удалась")
}

fn block(blocks: &Blocks, name: &str) -> BlockId {
    blocks
        .id(&Identifier::parse(name).expect("корректный id"))
        .expect("блок зарегистрирован")
}

fn мешить(view: &PaddedSection, blocks: &Blocks) -> MeshData {
    let mut mesh = MeshData::new();
    greedy_mesh(view, blocks, &Textures, &mut mesh);
    mesh
}

#[test]
fn пустая_секция_не_даёт_геометрии() {
    let blocks = реестр();
    let view = PaddedSection::new();
    let mesh = мешить(&view, &blocks);
    assert!(mesh.is_empty());
    assert_eq!(mesh.quad_count(), 0);
    assert!(mesh.indices.is_empty());
}

#[test]
fn один_блок_даёт_шесть_квадов() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    view.set(5, 5, 5, stone);

    let mesh = мешить(&view, &blocks);
    assert_eq!(mesh.quad_count(), 6, "у куба шесть видимых граней");
    assert_eq!(mesh.vertices.len(), 24, "по четыре вершины на квад");
    assert_eq!(mesh.indices.len(), 36, "по шесть индексов на квад");

    // Все шесть направлений присутствуют ровно по разу.
    let mut faces: Vec<u32> = mesh.vertices.iter().map(|v| v.face_index()).collect();
    faces.sort_unstable();
    faces.dedup();
    assert_eq!(faces.len(), 6, "должны быть все шесть направлений");
}

#[test]
fn плита_шестнадцать_на_шестнадцать_даёт_один_квад_сверху() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    for z in 0..SECTION_SIZE {
        for x in 0..SECTION_SIZE {
            view.set(x, 0, z, stone);
        }
    }

    let mesh = мешить(&view, &blocks);

    let верхних = mesh
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .count()
        / 4;
    assert_eq!(верхних, 1, "вся верхняя поверхность — один квад");

    let нижних = mesh
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::NegY.index() as u32)
        .count()
        / 4;
    assert_eq!(нижних, 1, "нижняя поверхность тоже один квад");

    // Боковые грани: по одному квадру на каждую из четырёх сторон,
    // слой высотой в один блок.
    assert_eq!(mesh.quad_count(), 6, "два горизонтальных и четыре боковых");
}

#[test]
fn два_разных_блока_не_склеиваются() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let dirt = block(&blocks, "test:dirt");
    let mut view = PaddedSection::new();
    for z in 0..SECTION_SIZE {
        for x in 0..SECTION_SIZE {
            view.set(x, 0, z, if x < 8 { stone } else { dirt });
        }
    }

    let mesh = мешить(&view, &blocks);
    let верхних = mesh
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .count()
        / 4;
    assert_eq!(верхних, 2, "камень и земля должны остаться разными квадами");

    // И текстуры у них разные.
    let текстуры: Vec<u32> = mesh
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .map(|v| v.texture())
        .collect();
    assert!(
        текстуры.iter().any(|t| *t != текстуры[0]),
        "у разных блоков должны быть разные текстуры"
    );
}

#[test]
fn грань_между_соседями_не_рисуется() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    view.set(5, 5, 5, stone);
    view.set(6, 5, 5, stone);

    let mesh = мешить(&view, &blocks);

    // Соприкасающиеся грани лежали бы в плоскости x = 6 и смотрели по ±X.
    let на_стыке = mesh
        .vertices
        .iter()
        .filter(|v| {
            let (x, _, _) = v.position();
            x == 6
                && (v.face_index() == Face::PosX.index() as u32
                    || v.face_index() == Face::NegX.index() as u32)
        })
        .count();
    assert_eq!(
        на_стыке, 0,
        "грань между соприкасающимися блоками не рисуется"
    );

    // Шесть, а не десять: оставшиеся грани ещё и склеиваются greedy —
    // верх, низ и обе стороны по Z становятся квадами 2x1.
    assert_eq!(mesh.quad_count(), 6);
}

#[test]
fn грань_на_границе_секции_скрыта_непрозрачным_соседом() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");

    // Блок у самого края секции, за границей — сосед из другой колонки.
    let mut with_neighbour = PaddedSection::new();
    with_neighbour.set(15, 5, 5, stone);
    with_neighbour.set(16, 5, 5, stone);
    let закрыт = мешить(&with_neighbour, &blocks).quad_count();

    let mut without = PaddedSection::new();
    without.set(15, 5, 5, stone);
    let открыт = мешить(&without, &blocks).quad_count();

    assert_eq!(открыт, 6, "без соседа видны все шесть граней");
    assert_eq!(
        закрыт, 5,
        "сосед за границей секции должен скрывать грань, иначе на стыке чанков будет лишняя стена"
    );
}

#[test]
fn прозрачный_сосед_не_скрывает_грань() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let glass = block(&blocks, "test:glass");

    let mut view = PaddedSection::new();
    view.set(5, 5, 5, stone);
    view.set(6, 5, 5, glass);

    let mesh = мешить(&view, &blocks);
    let камень = mesh
        .vertices
        .iter()
        .filter(|v| v.texture() / 8 == u32::from(stone.get()))
        .count();
    assert_eq!(камень / 4, 6, "у камня должны быть видны все грани");
}

#[test]
fn столб_склеивается_по_вертикали() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    for y in 0..SECTION_SIZE {
        view.set(5, y, 5, stone);
    }

    let mesh = мешить(&view, &blocks);
    // Четыре боковых квада на всю высоту плюс верх и низ.
    assert_eq!(
        mesh.quad_count(),
        6,
        "столб 1x16x1 должен дать шесть квадов"
    );
}

#[test]
fn полный_куб_секции_даёт_шесть_квадов() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                view.set(x, y, z, stone);
            }
        }
    }

    let mesh = мешить(&view, &blocks);
    assert_eq!(
        mesh.quad_count(),
        6,
        "сплошная секция — это шесть больших квадов, внутренние грани не нужны"
    );
}

#[test]
fn секция_окружённая_блоками_не_даёт_геометрии() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    // Заполняем и секцию, и всё поле соседей.
    for y in -1..=SECTION_SIZE {
        for z in -1..=SECTION_SIZE {
            for x in -1..=SECTION_SIZE {
                view.set(x, y, z, stone);
            }
        }
    }

    let mesh = мешить(&view, &blocks);
    assert!(
        mesh.is_empty(),
        "секция внутри сплошного камня невидима и геометрии не требует"
    );
}

#[test]
fn позиции_вершин_квада_ограничивают_блок() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    view.set(3, 4, 5, stone);

    let mesh = мешить(&view, &blocks);
    for vertex in &mesh.vertices {
        let (x, y, z) = vertex.position();
        assert!((3..=4).contains(&x), "x вне блока: {x}");
        assert!((4..=5).contains(&y), "y вне блока: {y}");
        assert!((5..=6).contains(&z), "z вне блока: {z}");
    }
}

#[test]
fn у_каждого_квада_четыре_разные_вершины() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    view.set(2, 2, 2, stone);

    let mesh = мешить(&view, &blocks);
    for quad in mesh.vertices.chunks(4) {
        let mut positions: Vec<(u32, u32, u32)> = quad.iter().map(|v| v.position()).collect();
        positions.sort_unstable();
        positions.dedup();
        assert_eq!(positions.len(), 4, "вершины квада совпали: {quad:?}");
    }
}

#[test]
fn индексы_указывают_на_существующие_вершины() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let dirt = block(&blocks, "test:dirt");
    let mut view = PaddedSection::new();
    for y in 0..8 {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                view.set(x, y, z, if (x + y + z) % 3 == 0 { stone } else { dirt });
            }
        }
    }

    let mesh = мешить(&view, &blocks);
    assert!(!mesh.is_empty());
    assert_eq!(mesh.indices.len(), mesh.quad_count() * 6);
    for index in &mesh.indices {
        assert!(
            (*index as usize) < mesh.vertices.len(),
            "индекс {index} за пределами {} вершин",
            mesh.vertices.len()
        );
    }
}

#[test]
fn ambient_occlusion_темнеет_в_углу() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut view = PaddedSection::new();
    // Пол и стена: угол между ними должен быть затенён сильнее середины пола.
    for z in 0..SECTION_SIZE {
        for x in 0..SECTION_SIZE {
            view.set(x, 0, z, stone);
        }
    }
    for z in 0..SECTION_SIZE {
        for y in 1..4 {
            view.set(0, y, z, stone);
        }
    }

    let mesh = мешить(&view, &blocks);
    let верх: Vec<_> = mesh
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .collect();
    assert!(!верх.is_empty());

    let самый_тёмный = верх.iter().map(|v| v.ao()).min().unwrap_or(3);
    let самый_светлый = верх.iter().map(|v| v.ao()).max().unwrap_or(0);
    assert!(
        самый_тёмный < самый_светлый,
        "у стены должно быть темнее, чем в открытом месте: {самый_тёмный} против {самый_светлый}"
    );
}

#[test]
fn разное_затенение_мешает_склейке() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");

    // Ровный пол: верх склеивается в один квад.
    let mut ровный = PaddedSection::new();
    for z in 0..SECTION_SIZE {
        for x in 0..SECTION_SIZE {
            ровный.set(x, 0, z, stone);
        }
    }
    let ровных = мешить(&ровный, &blocks)
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .count()
        / 4;
    assert_eq!(ровных, 1);

    // Тот же пол со столбиком посередине: вокруг него затенение другое,
    // и один квад уже не получится.
    let mut со_столбом = ровный.clone();
    со_столбом.set(8, 1, 8, stone);
    let со_столбом_квадов = мешить(&со_столбом, &blocks)
        .vertices
        .iter()
        .filter(|v| v.face_index() == Face::PosY.index() as u32)
        .count()
        / 4;
    assert!(
        со_столбом_квадов > 1,
        "затенение вокруг столба должно разбить поверхность на части"
    );
}

#[test]
fn буфер_переиспользуется_без_мусора() {
    let blocks = реестр();
    let stone = block(&blocks, "test:stone");
    let mut mesh = MeshData::new();

    let mut view = PaddedSection::new();
    view.set(1, 1, 1, stone);
    greedy_mesh(&view, &blocks, &Textures, &mut mesh);
    let первый = mesh.quad_count();

    // Второй прогон в тот же буфер не должен накапливать геометрию.
    greedy_mesh(&view, &blocks, &Textures, &mut mesh);
    assert_eq!(
        mesh.quad_count(),
        первый,
        "буфер не очищается между вызовами"
    );

    let пустая = PaddedSection::new();
    greedy_mesh(&пустая, &blocks, &Textures, &mut mesh);
    assert!(mesh.is_empty(), "после пустой секции буфер должен опустеть");
}
