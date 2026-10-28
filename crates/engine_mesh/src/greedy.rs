//! Greedy meshing: склейка соседних одинаковых граней в прямоугольники.
//!
//! Наивный мешер даёт по квадру на каждую видимую грань вокселя. Greedy склеивает
//! соседние грани, у которых совпадают блок, направление и затенение, в один
//! прямоугольник: ровная поверхность 16×16 превращается из 256 квадов в один.
//!
//! Склеивать можно только полностью одинаковые грани. Разные блоки дали бы одну
//! текстуру на двоих, а разное ambient occlusion — неверную растушёвку, потому что
//! затенение интерполируется между вершинами квада.

use crate::vertex::{AO_MAX, PackedVertex};
use crate::view::PaddedSection;
use engine_world::{BlockId, Blocks, Face, SECTION_SIZE};

/// Готовая геометрия секции.
#[derive(Debug, Clone, Default)]
pub struct MeshData {
    /// Вершины по четыре на квад.
    pub vertices: Vec<PackedVertex>,
    /// Индексы по шесть на квад.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Пустая геометрия.
    pub fn new() -> Self {
        Self::default()
    }

    /// Очистить, сохранив выделенную память.
    ///
    /// Буферы переиспользуются между задачами: выделять по два вектора на каждую
    /// из тысяч секций — это аллокации в самом горячем месте.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Сколько квадов в геометрии.
    #[inline]
    pub fn quad_count(&self) -> usize {
        self.vertices.len() / 4
    }

    /// Пуста ли геометрия.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Чем описывается грань для склейки.
///
/// Две грани сливаются, только если их ключи равны целиком.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaceKey {
    block: BlockId,
    /// Ambient occlusion четырёх углов.
    ao: [u8; 4],
}

/// Источник текстур для блоков.
///
/// Мешер не знает про атлас: ему нужен только номер слоя. Реализация живёт в
/// рендере, а в тестах подменяется заглушкой.
pub trait TextureLookup {
    /// Индекс текстуры грани блока в texture array.
    fn texture_index(&self, block: BlockId, face: Face) -> u32;
}

/// Построить геометрию секции.
///
/// `out` очищается и переиспользуется.
pub fn greedy_mesh(
    view: &PaddedSection,
    blocks: &Blocks,
    textures: &impl TextureLookup,
    out: &mut MeshData,
) {
    out.clear();

    // Пустая секция не даёт граней, а проход по шести направлениям всё равно
    // стоил бы 24576 проверок. В колонке таких секций большинство.
    if !view.has_blocks() {
        return;
    }

    for face in Face::ALL {
        mesh_face(view, blocks, textures, face, out);
    }
}

/// Построить геометрию одной грани-направления.
fn mesh_face(
    view: &PaddedSection,
    blocks: &Blocks,
    textures: &impl TextureLookup,
    face: Face,
    out: &mut MeshData,
) {
    let size = SECTION_SIZE;
    // Маска слоя: что видно и с каким затенением.
    let mut mask: [Option<FaceKey>; (SECTION_SIZE * SECTION_SIZE) as usize] =
        [None; (SECTION_SIZE * SECTION_SIZE) as usize];

    for layer in 0..size {
        let mut any = false;
        for v in 0..size {
            for u in 0..size {
                let (x, y, z) = face_coords(face, layer, u, v);
                let block = view.get(x, y, z);
                if block.is_air() || blocks.is_empty_block(block) {
                    mask[(v * size + u) as usize] = None;
                    continue;
                }

                // Грань видна, только если сосед её не перекрывает.
                let (dx, dy, dz) = face.offset();
                let neighbour = view.get(x + dx, y + dy, z + dz);
                if blocks.is_opaque(neighbour) {
                    mask[(v * size + u) as usize] = None;
                    continue;
                }

                mask[(v * size + u) as usize] = Some(FaceKey {
                    block,
                    ao: corner_ao(view, blocks, face, x, y, z),
                });
                any = true;
            }
        }

        if any {
            emit_layer(&mut mask, face, layer, textures, out);
        }
    }
}

/// Склеить маску слоя в прямоугольники и выдать квады.
fn emit_layer(
    mask: &mut [Option<FaceKey>],
    face: Face,
    layer: i32,
    textures: &impl TextureLookup,
    out: &mut MeshData,
) {
    let size = SECTION_SIZE;

    for v in 0..size {
        let mut u = 0;
        while u < size {
            let index = (v * size + u) as usize;
            let Some(key) = mask[index] else {
                u += 1;
                continue;
            };

            // Растём вправо, пока ключ тот же.
            let mut width = 1;
            while u + width < size && mask[(v * size + u + width) as usize] == Some(key) {
                width += 1;
            }

            // Растём вниз, пока вся строка шириной width совпадает.
            let mut height = 1;
            'grow: while v + height < size {
                for offset in 0..width {
                    if mask[((v + height) * size + u + offset) as usize] != Some(key) {
                        break 'grow;
                    }
                }
                height += 1;
            }

            for row in 0..height {
                for column in 0..width {
                    mask[((v + row) * size + u + column) as usize] = None;
                }
            }

            emit_quad(face, layer, u, v, width, height, &key, textures, out);
            u += width;
        }
    }
}

/// Выдать один квад.
#[allow(clippy::too_many_arguments)]
fn emit_quad(
    face: Face,
    layer: i32,
    u: i32,
    v: i32,
    width: i32,
    height: i32,
    key: &FaceKey,
    textures: &impl TextureLookup,
    out: &mut MeshData,
) {
    let texture = textures.texture_index(key.block, face);
    let base = out.vertices.len() as u32;

    // Четыре угла квада в координатах слоя, по часовой стрелке при взгляде снаружи.
    let corners = [
        (u, v),
        (u + width, v),
        (u + width, v + height),
        (u, v + height),
    ];
    let uvs = [(0, 0), (width, 0), (width, height), (0, height)];

    for (corner, (cu, cv)) in corners.iter().enumerate() {
        let (x, y, z) = quad_corner(face, layer, *cu, *cv);
        let (tu, tv) = uvs[corner];
        out.vertices.push(PackedVertex::new(
            (x as u32, y as u32, z as u32),
            key.ao[corner],
            face,
            texture,
            (tu as u32, tv as u32),
        ));
    }

    // Если противоположные углы затенены по-разному, диагональ надо развернуть:
    // иначе на квадрате видно неверный градиент — классический артефакт AO.
    let flip = key.ao[0] as u32 + key.ao[2] as u32 > key.ao[1] as u32 + key.ao[3] as u32;
    if flip {
        out.indices
            .extend_from_slice(&[base + 1, base + 2, base + 3, base + 3, base, base + 1]);
    } else {
        out.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
    }
}

/// Координаты вокселя по номеру слоя и позиции в нём.
#[inline]
fn face_coords(face: Face, layer: i32, u: i32, v: i32) -> (i32, i32, i32) {
    match face {
        Face::PosX | Face::NegX => (layer, v, u),
        Face::PosY | Face::NegY => (u, layer, v),
        Face::PosZ | Face::NegZ => (u, v, layer),
    }
}

/// Координаты угла квада. Для положительных граней плоскость сдвинута на блок.
#[inline]
fn quad_corner(face: Face, layer: i32, u: i32, v: i32) -> (i32, i32, i32) {
    let plane = match face {
        Face::PosX | Face::PosY | Face::PosZ => layer + 1,
        Face::NegX | Face::NegY | Face::NegZ => layer,
    };
    match face {
        Face::PosX | Face::NegX => (plane, v, u),
        Face::PosY | Face::NegY => (u, plane, v),
        Face::PosZ | Face::NegZ => (u, v, plane),
    }
}

/// Ambient occlusion четырёх углов грани.
///
/// Для каждого угла смотрятся три соседа в плоскости перед гранью: два по сторонам
/// и один по диагонали. Чем больше из них непрозрачны, тем темнее угол. Если оба
/// боковых закрыты, диагональ уже не важна — угол и так полностью затенён.
fn corner_ao(view: &PaddedSection, blocks: &Blocks, face: Face, x: i32, y: i32, z: i32) -> [u8; 4] {
    let (dx, dy, dz) = face.offset();
    // Плоскость перед гранью.
    let (fx, fy, fz) = (x + dx, y + dy, z + dz);
    let (au, av) = face_axes(face);

    let mut result = [AO_MAX; 4];
    // Углы в том же порядке, что и вершины квада.
    let offsets = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

    for (corner, (ou, ov)) in offsets.iter().enumerate() {
        let side1 = sample(view, blocks, fx, fy, fz, au, *ou);
        let side2 = sample(view, blocks, fx, fy, fz, av, *ov);
        let diagonal = sample_two(view, blocks, fx, fy, fz, au, *ou, av, *ov);

        result[corner] = if side1 && side2 {
            0
        } else {
            AO_MAX - u8::from(side1) - u8::from(side2) - u8::from(diagonal)
        };
    }
    result
}

/// Оси, вдоль которых идут стороны грани.
#[inline]
fn face_axes(face: Face) -> ((i32, i32, i32), (i32, i32, i32)) {
    match face {
        Face::PosX | Face::NegX => ((0, 0, 1), (0, 1, 0)),
        Face::PosY | Face::NegY => ((1, 0, 0), (0, 0, 1)),
        Face::PosZ | Face::NegZ => ((1, 0, 0), (0, 1, 0)),
    }
}

/// Непрозрачен ли сосед, сдвинутый на одну ось.
#[inline]
fn sample(
    view: &PaddedSection,
    blocks: &Blocks,
    x: i32,
    y: i32,
    z: i32,
    axis: (i32, i32, i32),
    sign: i32,
) -> bool {
    let (ax, ay, az) = axis;
    let (nx, ny, nz) = (x + ax * sign, y + ay * sign, z + az * sign);
    if !in_view(nx, ny, nz) {
        return false;
    }
    blocks.is_opaque(view.get(nx, ny, nz))
}

/// Непрозрачен ли сосед по диагонали.
#[allow(clippy::too_many_arguments)]
#[inline]
fn sample_two(
    view: &PaddedSection,
    blocks: &Blocks,
    x: i32,
    y: i32,
    z: i32,
    first: (i32, i32, i32),
    first_sign: i32,
    second: (i32, i32, i32),
    second_sign: i32,
) -> bool {
    let (ax, ay, az) = first;
    let (bx, by, bz) = second;
    let nx = x + ax * first_sign + bx * second_sign;
    let ny = y + ay * first_sign + by * second_sign;
    let nz = z + az * first_sign + bz * second_sign;
    if !in_view(nx, ny, nz) {
        return false;
    }
    blocks.is_opaque(view.get(nx, ny, nz))
}

/// Попадает ли координата в снимок.
///
/// Углы грани у края секции смотрят на два блока наружу, а снимок знает только
/// один слой соседей. Такие соседи считаются прозрачными: это даёт чуть более
/// светлый угол на самой границе вместо паники или лишнего слоя в снимке.
#[inline]
fn in_view(x: i32, y: i32, z: i32) -> bool {
    (-1..=SECTION_SIZE).contains(&x)
        && (-1..=SECTION_SIZE).contains(&y)
        && (-1..=SECTION_SIZE).contains(&z)
}
