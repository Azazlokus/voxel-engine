//! Тесты загрузчика на настоящих файлах.
//!
//! Загрузчик работает с файловой системой, поэтому и проверять его нужно на ней:
//! мок пути и расширения не поймал бы ни регистр, ни сортировку, ни пропуск
//! посторонних файлов.

use engine_assets::{AssetError, ContentRoot, load_blocks};
use engine_world::Face;
use std::path::{Path, PathBuf};

/// Временная папка, убирающая себя за собой.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        // Имя включает id процесса и имя теста: тесты идут параллельно,
        // и общая папка привела бы к гонке.
        let path =
            std::env::temp_dir().join(format!("voxel-engine-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("временная папка создаётся");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("родительская папка создаётся");
        }
        std::fs::write(path, content).expect("файл записывается");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn читаются_все_описания_пространства_имён() {
    let dir = TempDir::new("read-all");
    dir.write(
        "base/blocks/stone.ron",
        r#"(textures: All("base:stone"), hardness: 1.5)"#,
    );
    dir.write(
        "base/blocks/grass.ron",
        r#"(textures: Layered(top: "base:grass_top", side: "base:grass_side", bottom: "base:dirt"), hardness: 0.6)"#,
    );

    let root = ContentRoot::new(dir.path());
    let blocks = load_blocks(&root, "base").expect("описания читаются");

    assert_eq!(blocks.len(), 2);
    // Порядок отсортирован по пути, а не как отдала файловая система.
    assert_eq!(blocks[0].id.as_str(), "base:grass");
    assert_eq!(blocks[1].id.as_str(), "base:stone");
    assert_eq!(
        blocks[0].definition.texture(Face::PosY),
        Some("base:grass_top")
    );
    assert_eq!(blocks[1].definition.hardness, 1.5);
}

#[test]
fn идентификатор_берётся_из_имени_файла() {
    let dir = TempDir::new("id-from-name");
    dir.write(
        "mymod/blocks/copper_ore.ron",
        r#"(textures: All("mymod:copper_ore"))"#,
    );

    let root = ContentRoot::new(dir.path());
    let blocks = load_blocks(&root, "mymod").expect("описания читаются");
    assert_eq!(blocks[0].id.as_str(), "mymod:copper_ore");
    assert_eq!(blocks[0].id.namespace(), "mymod");
}

#[test]
fn посторонние_файлы_игнорируются() {
    let dir = TempDir::new("ignore-others");
    dir.write("base/blocks/stone.ron", r#"(textures: All("base:stone"))"#);
    dir.write("base/blocks/readme.txt", "не описание блока");
    dir.write("base/blocks/stone.ron.bak", "мусор от редактора");

    let root = ContentRoot::new(dir.path());
    let blocks = load_blocks(&root, "base").expect("описания читаются");
    assert_eq!(blocks.len(), 1, "только .ron должны читаться");
}

#[test]
fn отсутствие_папки_не_ошибка() {
    // Пространство имён может содержать только текстуры.
    let dir = TempDir::new("no-dir");
    dir.write("base/textures/stone.png", "не настоящий png");

    let root = ContentRoot::new(dir.path());
    let blocks = load_blocks(&root, "base").expect("отсутствие папки blocks допустимо");
    assert!(blocks.is_empty());

    let blocks = load_blocks(&root, "nonexistent").expect("отсутствие пространства имён допустимо");
    assert!(blocks.is_empty());
}

#[test]
fn битый_ron_сообщает_путь() {
    let dir = TempDir::new("broken-ron");
    dir.write("base/blocks/stone.ron", r#"(textures: All("base:stone""#);

    let root = ContentRoot::new(dir.path());
    let err = load_blocks(&root, "base").expect_err("битый RON должен быть ошибкой");

    match &err {
        AssetError::Ron { path, .. } => {
            assert!(
                path.ends_with("stone.ron"),
                "путь должен указывать на файл, получено {path:?}"
            );
        }
        other => panic!("ожидалась ошибка разбора, получено {other:?}"),
    }
    // Сообщение должно содержать имя файла: иначе оно бесполезно при десятках файлов.
    assert!(err.to_string().contains("stone.ron"), "{err}");
}

#[test]
fn неверное_имя_файла_отвергается() {
    let dir = TempDir::new("bad-name");
    // Заглавные буквы в идентификаторах запрещены.
    dir.write("base/blocks/Stone.ron", r#"(textures: All("base:stone"))"#);

    let root = ContentRoot::new(dir.path());
    let err = load_blocks(&root, "base").expect_err("заглавные буквы недопустимы");
    assert!(matches!(err, AssetError::BadFileName { .. }), "{err:?}");
}

#[test]
fn неверная_текстура_отвергается_на_загрузке() {
    let dir = TempDir::new("bad-texture");
    // Текстура без пространства имён: ошибка должна всплыть здесь, а не в рендере.
    dir.write("base/blocks/stone.ron", r#"(textures: All("stone"))"#);

    let root = ContentRoot::new(dir.path());
    let err = load_blocks(&root, "base").expect_err("текстура без namespace недопустима");
    match &err {
        AssetError::BadTexture { texture, path, .. } => {
            assert_eq!(texture, "stone");
            assert!(path.ends_with("stone.ron"));
        }
        other => panic!("ожидалась ошибка текстуры, получено {other:?}"),
    }
}

#[test]
fn непустой_блок_без_текстур_отвергается() {
    let dir = TempDir::new("missing-textures");
    dir.write("base/blocks/stone.ron", r#"(hardness: 1.0)"#);

    let root = ContentRoot::new(dir.path());
    let err = load_blocks(&root, "base").expect_err("непустому блоку нужны текстуры");
    assert!(matches!(err, AssetError::MissingTextures { .. }), "{err:?}");
}

#[test]
fn пустой_блок_без_текстур_допустим() {
    // Воздух: текстур нет и быть не должно.
    let dir = TempDir::new("empty-no-textures");
    dir.write("base/blocks/air.ron", r#"(empty: true, opaque: false)"#);

    let root = ContentRoot::new(dir.path());
    let blocks = load_blocks(&root, "base").expect("пустой блок без текстур допустим");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id.as_str(), "base:air");
    assert!(blocks[0].definition.textures.is_none());
}

#[test]
fn пути_внутри_корня_собираются_верно() {
    let root = ContentRoot::new("assets");
    assert_eq!(root.blocks_dir("base"), Path::new("assets/base/blocks"));
    assert_eq!(root.textures_dir("base"), Path::new("assets/base/textures"));
    assert_eq!(root.namespace_dir("mymod"), Path::new("assets/mymod"));
    assert_eq!(ContentRoot::default().path(), Path::new("assets"));
}
