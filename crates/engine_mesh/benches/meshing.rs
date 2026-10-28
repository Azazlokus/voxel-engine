//! Бенчмарк greedy meshing.
//!
//! Главный случай — типичная секция настоящего мира: поверхность с травой,
//! землёй и камнем. Синтетические крайности (пустая, сплошная) нужны рядом как
//! границы: по ним видно, сколько стоит сам проход по секции, а сколько склейка.

use criterion::{Criterion, criterion_group, criterion_main};
use engine_mesh::{MeshData, PaddedSection, TextureLookup, greedy_mesh};
use engine_registry::Identifier;
use engine_world::{
    AIR_ID, BlockId, BlockProperties, BlockRegistry, Blocks, ChunkPos, Column, Face, SECTION_SIZE,
    WorldGenerator,
};
use engine_worldgen::TerrainGenerator;
use std::hint::black_box;

struct Textures;

impl TextureLookup for Textures {
    fn texture_index(&self, block: BlockId, face: Face) -> u32 {
        u32::from(block.get()) * 8 + face.index() as u32
    }
}

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

/// Снимок секции, в которую попадает поверхность сгенерированного мира.
fn типичная_секция(blocks: &Blocks) -> PaddedSection {
    let generator = TerrainGenerator::new(0x5eed);
    let position = ChunkPos::new(0, 0);
    let mut column = Column::new(position);
    generator.generate_column(position, blocks, &mut column);

    // Секция с поверхностью: в ней и камень, и трава, и воздух.
    let surface = column.highest_block_y().unwrap_or(64);
    let index = Column::section_index(surface).unwrap_or(0);

    let mut view = PaddedSection::new();
    view.fill_from_column(&column, index);
    view
}

fn сплошная_секция(block: BlockId) -> PaddedSection {
    let mut view = PaddedSection::new();
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                view.set(x, y, z, block);
            }
        }
    }
    view
}

/// Шахматка: худший случай для склейки — ни одна пара граней не сливается.
fn шахматная_секция(a: BlockId, b: BlockId) -> PaddedSection {
    let mut view = PaddedSection::new();
    for y in 0..SECTION_SIZE {
        for z in 0..SECTION_SIZE {
            for x in 0..SECTION_SIZE {
                if (x + y + z) % 2 == 0 {
                    view.set(x, y, z, if (x + y) % 4 == 0 { a } else { b });
                }
            }
        }
    }
    view
}

fn bench_meshing(c: &mut Criterion) {
    let blocks = blocks();
    let stone = blocks
        .id(&Identifier::parse("base:stone").expect("корректный id"))
        .expect("камень есть");
    let dirt = blocks
        .id(&Identifier::parse("base:dirt").expect("корректный id"))
        .expect("земля есть");

    let типичная = типичная_секция(&blocks);
    let сплошная = сплошная_секция(stone);
    let шахматка = шахматная_секция(stone, dirt);
    let пустая = PaddedSection::new();

    let mut mesh = MeshData::new();
    let mut group = c.benchmark_group("meshing");

    group.bench_function("типичная секция", |b| {
        b.iter(|| {
            greedy_mesh(black_box(&типичная), &blocks, &Textures, &mut mesh);
            black_box(mesh.quad_count())
        });
    });

    group.bench_function("сплошная секция", |b| {
        b.iter(|| {
            greedy_mesh(black_box(&сплошная), &blocks, &Textures, &mut mesh);
            black_box(mesh.quad_count())
        });
    });

    group.bench_function("шахматка", |b| {
        b.iter(|| {
            greedy_mesh(black_box(&шахматка), &blocks, &Textures, &mut mesh);
            black_box(mesh.quad_count())
        });
    });

    group.bench_function("пустая секция", |b| {
        b.iter(|| {
            greedy_mesh(black_box(&пустая), &blocks, &Textures, &mut mesh);
            black_box(mesh.quad_count())
        });
    });

    group.finish();
}

criterion_group!(benches, bench_meshing);
criterion_main!(benches);
