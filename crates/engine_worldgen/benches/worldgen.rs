//! Бенчмарк генерации колонки.
//!
//! Меряется то, что в стриминге стоит дороже всего: полная генерация колонки
//! 16×384×16 вместе с деревьями и сжатием секций.

use criterion::{Criterion, criterion_group, criterion_main};
use engine_registry::Identifier;
use engine_world::{
    AIR_ID, BlockProperties, BlockRegistry, Blocks, ChunkPos, Column, WorldGenerator,
};
use engine_worldgen::TerrainGenerator;
use std::hint::black_box;

/// Реестр блоков для бенчмарка: собирается вручную, чтобы не зависеть от
/// файлов контента и не мерить заодно чтение с диска.
fn blocks() -> Blocks {
    let mut registry = BlockRegistry::new();
    for name in [
        AIR_ID,
        "base:stone",
        "base:dirt",
        "base:grass",
        "base:sand",
        "base:water",
        "base:log",
        "base:leaves",
    ] {
        let properties = if name == AIR_ID {
            BlockProperties::AIR
        } else {
            BlockProperties::SOLID
        };
        registry
            .register(Identifier::parse(name).expect("корректный id"), properties)
            .expect("блок регистрируется");
    }
    registry.freeze().expect("заморозка удалась")
}

fn bench_column(c: &mut Criterion) {
    let blocks = blocks();
    let generator = TerrainGenerator::new(0x5eed);

    // Прогрев: первый вызов разрешает палитру, и мерить его вместе с остальными
    // значит завысить медиану.
    let mut warmup = Column::new(ChunkPos::new(1000, 1000));
    generator.generate_column(ChunkPos::new(1000, 1000), &blocks, &mut warmup);

    let mut group = c.benchmark_group("worldgen");

    group.bench_function("колонка целиком", |b| {
        let mut counter = 0i32;
        b.iter(|| {
            // Разные координаты на каждой итерации: одна и та же колонка
            // прогрела бы кеш и дала заниженный результат.
            counter += 1;
            let position = ChunkPos::new(counter % 64, counter / 64);
            let mut column = Column::new(position);
            generator.generate_column(position, &blocks, black_box(&mut column));
            black_box(column.data_bytes())
        });
    });

    group.bench_function("карта высот 16x16", |b| {
        let heights = generator.heights();
        b.iter(|| {
            let mut sum = 0i64;
            for z in 0..16 {
                for x in 0..16 {
                    sum += i64::from(heights.height(black_box(x), black_box(z)));
                }
            }
            black_box(sum)
        });
    });

    group.finish();
}

criterion_group!(benches, bench_column);
criterion_main!(benches);
