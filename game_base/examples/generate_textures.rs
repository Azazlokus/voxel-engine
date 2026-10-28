//! Генератор текстур базового контента.
//!
//! Запуск: `cargo run -p game_base --example generate_textures`
//!
//! Текстуры 16×16 генерируются процедурно и кладутся в `assets/base/textures/`,
//! после чего попадают в репозиторий как обычные файлы. Генератор — отдельный
//! пример, а не часть движка: в рантайме кодировать PNG незачем, и благодаря этому
//! `image` остаётся dev-зависимостью и не попадает в граф зависимостей игры.
//!
//! Генерация детерминирована: одно и то же имя текстуры всегда даёт один и тот же
//! результат, поэтому повторный запуск не создаёт шума в диффе.

use std::path::{Path, PathBuf};

/// Сторона текстуры в пикселях.
const SIZE: u32 = 16;

/// Описание того, как генерируется одна текстура.
struct Texture {
    /// Имя файла без расширения.
    name: &'static str,
    /// Базовый цвет RGB.
    color: [u8; 3],
    /// Насколько сильно шум меняет яркость, в единицах 0..255.
    contrast: i16,
    /// Узор поверх шума.
    pattern: Pattern,
}

/// Узор, отличающий текстуру от однородного шума.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pattern {
    /// Только шум.
    Noise,
    /// Крупные пятна: земля, гравий.
    Blobs,
    /// Вертикальные волокна: бок бревна.
    Fibers,
    /// Концентрические кольца: торец бревна.
    Rings,
    /// Трава сверху: шум плюс редкие более светлые травинки.
    Grass,
    /// Трава на боковой грани: земля снизу, трава сверху.
    GrassSide,
    /// Горизонтальные полосы: вода.
    Waves,
}

const TEXTURES: &[Texture] = &[
    Texture {
        name: "stone",
        color: [128, 128, 132],
        contrast: 18,
        pattern: Pattern::Noise,
    },
    Texture {
        name: "dirt",
        color: [120, 85, 58],
        contrast: 22,
        pattern: Pattern::Blobs,
    },
    Texture {
        name: "grass_top",
        color: [92, 148, 66],
        contrast: 18,
        pattern: Pattern::Grass,
    },
    Texture {
        name: "grass_side",
        color: [120, 85, 58],
        contrast: 20,
        pattern: Pattern::GrassSide,
    },
    Texture {
        name: "sand",
        color: [214, 202, 150],
        contrast: 14,
        pattern: Pattern::Noise,
    },
    Texture {
        name: "water",
        color: [58, 104, 180],
        contrast: 12,
        pattern: Pattern::Waves,
    },
    Texture {
        name: "log_side",
        color: [104, 76, 46],
        contrast: 20,
        pattern: Pattern::Fibers,
    },
    Texture {
        name: "log_top",
        color: [148, 116, 74],
        contrast: 16,
        pattern: Pattern::Rings,
    },
    Texture {
        name: "leaves",
        color: [58, 112, 48],
        contrast: 30,
        pattern: Pattern::Blobs,
    },
];

fn main() {
    let dir = textures_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!("не удалось создать {}: {err}", dir.display());
        std::process::exit(1);
    }

    for texture in TEXTURES {
        let image = render(texture);
        let path = dir.join(format!("{}.png", texture.name));
        match image.save(&path) {
            Ok(()) => println!("записано {}", path.display()),
            Err(err) => {
                eprintln!("не удалось записать {}: {err}", path.display());
                std::process::exit(1);
            }
        }
    }
    println!("готово: {} текстур в {}", TEXTURES.len(), dir.display());
}

/// Куда писать: `assets/base/textures` относительно корня workspace.
fn textures_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR указывает на game_base, корень workspace — на уровень выше.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().unwrap_or(manifest);
    root.join("assets").join("base").join("textures")
}

/// Нарисовать текстуру.
fn render(texture: &Texture) -> image::RgbaImage {
    // Seed из имени: текстура не меняется между запусками, поэтому повторная
    // генерация не создаёт шума в диффе.
    let seed = hash_str(texture.name);

    image::RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        let noise = value_noise(seed, x, y);
        let mut delta = ((noise * 2.0 - 1.0) * f32::from(texture.contrast)) as i16;
        let mut color = texture.color;

        match texture.pattern {
            Pattern::Noise => {}
            Pattern::Blobs => {
                // Шум меньшей частоты: пятна размером в несколько пикселей.
                let coarse = value_noise(seed ^ 0x9e37, x / 4, y / 4);
                delta += ((coarse - 0.5) * 36.0) as i16;
            }
            Pattern::Fibers => {
                let fiber = value_noise(seed ^ 0x51ed, x, 0);
                delta += ((fiber - 0.5) * 34.0) as i16;
            }
            Pattern::Rings => {
                let center = (SIZE as f32 - 1.0) / 2.0;
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let radius = (dx * dx + dy * dy).sqrt();
                // Кольца через каждые ~2 пикселя.
                let ring = (radius * 1.6).sin();
                delta += (ring * 16.0) as i16;
            }
            Pattern::Grass => {
                if value_noise(seed ^ 0x2f1b, x, y) > 0.82 {
                    delta += 26;
                }
            }
            Pattern::GrassSide => {
                // Верхние 5 пикселей — трава, ниже земля, с неровной границей.
                let edge = 3 + (value_noise(seed ^ 0x77af, x, 0) * 3.0) as u32;
                if y < edge {
                    color = [92, 148, 66];
                }
            }
            Pattern::Waves => {
                let wave = ((y as f32) * 1.2).sin();
                delta += (wave * 10.0) as i16;
            }
        }

        image::Rgba([
            shift(color[0], delta),
            shift(color[1], delta),
            shift(color[2], delta),
            255,
        ])
    })
}

/// Сдвинуть канал яркости с обрезкой по границам.
fn shift(channel: u8, delta: i16) -> u8 {
    (i16::from(channel) + delta).clamp(0, 255) as u8
}

/// Шум значения в `0.0..1.0` по целым координатам.
///
/// Своя реализация вместо крейта: нужен детерминизм под нашим контролем и ноль
/// зависимостей ради девяти картинок.
fn value_noise(seed: u32, x: u32, y: u32) -> f32 {
    let mut h = seed ^ x.wrapping_mul(0x85eb_ca6b) ^ y.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Хеш строки в seed.
fn hash_str(text: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for byte in text.as_bytes() {
        h ^= u32::from(*byte);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}
