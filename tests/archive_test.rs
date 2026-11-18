//! 智能归档（M4）集成测试：临时目录扫描建库，验证
//! 目录树预览与落盘一致、复制模式保留源文件、撤销恢复原路径、冲突不覆盖、
//! 4 种规则命名、缺失元数据兜底、操作日志与 settings 持久化。
//! 绝不触碰真实相册。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use akashic::core::config::Paths;
use akashic::core::model::OpKind;
use akashic::core::ops::archive::{self, ArchiveMode, ArchiveRule};
use akashic::core::{db, scanner};
use image::{Rgb, RgbImage};
use rusqlite::{params, Connection};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(name: &str) -> PathBuf {
    let uniq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "akashic_archive_{}_{}_{}",
        name,
        std::process::id(),
        uniq
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_jpeg(path: &PathBuf, r: u8, g: u8, b: u8) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    RgbImage::from_fn(20, 20, |_, _| Rgb([r, g, b])).save(path).unwrap();
}

/// 扫描 n 张图片入索引，并写入可控的日期/标签/地点元数据。
fn setup(n: usize) -> (Paths, Connection, PathBuf, Vec<i64>) {
    let base = temp_dir(&format!("setup_{}", n));
    let photos = base.join("photos");
    fs::create_dir_all(&photos).unwrap();
    for i in 1..=n {
        make_jpeg(&photos.join(format!("IMG_{:04}.jpg", i)), 200, 100, 50);
    }
    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    scanner::scan_root(&paths, &conn, &photos, None).unwrap();
    let ids: Vec<i64> = db::list_photos(&conn)
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    for (i, id) in ids.iter().enumerate() {
        let date = format!("2024-{:02}-15 09:45:00", (i % 3) + 1);
        conn.execute(
            "UPDATE photos SET date_taken=?1, tags='旅行,风景', location='富士山' WHERE id=?2",
            params![date, id],
        )
        .unwrap();
    }
    (paths, conn, photos, ids)
}

/// 归档目标目录：与照片目录同级，避免归档目录被扫描根包含。
fn dest_of(paths: &Paths) -> PathBuf {
    paths.data_dir.parent().unwrap().join("归档")
}

fn cleanup(paths: &Paths, conn: Connection) {
    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());
}

#[test]
fn move_archive_matches_preview_tree() {
    let (paths, conn, _photos, ids) = setup(4);
    let dest = dest_of(&paths);

    let p = archive::plan(&conn, &ids, ArchiveRule::ByYearMonth, ArchiveMode::Move, &dest).unwrap();
    assert_eq!(p.total_files, 4);
    assert_eq!(p.conflict_count, 0);
    assert_eq!(p.tree.iter().map(|n| n.total_count).sum::<usize>(), 4);
    assert_eq!(p.tree[0].name, "2024");

    // 预览的相对路径必须能还原成实际目标路径
    for item in &p.items {
        let expected = item
            .relative
            .split('/')
            .fold(dest.clone(), |acc, part| acc.join(part));
        assert_eq!(item.to_path, expected, "目录树预览须与落盘路径一致");
    }

    let r = archive::execute(&conn, &p).unwrap();
    assert_eq!(r.count, 4);
    assert_eq!(r.dest_root, dest);
    for item in &p.items {
        assert!(item.to_path.exists(), "目标应存在: {}", item.to_path.display());
        assert!(!item.from_path.exists(), "移动模式源文件应已移走");
        assert!(db::get_photo_by_path(&conn, &item.to_path).unwrap().is_some(), "索引应指向新路径");
    }
    assert_eq!(db::count_photos(&conn).unwrap(), 4, "记录数不变");

    let ops = db::list_ops(&conn, 10).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, OpKind::ArchiveMove);
    assert_eq!(ops[0].item_count, 4);

    cleanup(&paths, conn);
}

#[test]
fn copy_mode_preserves_source_and_undo_removes_only_copy() {
    let (paths, conn, _photos, ids) = setup(3);
    let dest = dest_of(&paths);

    let p = archive::plan(&conn, &ids, ArchiveRule::ByYear, ArchiveMode::Copy, &dest).unwrap();
    let r = archive::execute(&conn, &p).unwrap();
    assert_eq!(r.count, 3);
    for item in &p.items {
        assert!(item.from_path.exists(), "复制模式必须保留源文件");
        assert!(item.to_path.exists(), "归档副本应存在");
    }
    // 源文件 + 副本都进入索引
    assert_eq!(db::count_photos(&conn).unwrap(), 6);
    assert_eq!(
        db::list_ops(&conn, 10).unwrap()[0].kind,
        OpKind::ArchiveCopy
    );

    let n = archive::undo(&conn, r.op_id).unwrap();
    assert_eq!(n, 3);
    for item in &p.items {
        assert!(item.from_path.exists(), "撤销后源文件仍在");
        assert!(!item.to_path.exists(), "撤销只删除本次生成的副本");
        assert!(db::get_photo_by_path(&conn, &item.from_path).unwrap().is_some());
    }
    assert_eq!(db::count_photos(&conn).unwrap(), 3, "副本索引应被删除");
    assert!(db::list_ops(&conn, 10).unwrap().is_empty());

    cleanup(&paths, conn);
}

#[test]
fn undo_move_restores_original_paths_and_index() {
    let (paths, conn, _photos, ids) = setup(3);
    let dest = dest_of(&paths);

    let p = archive::plan(&conn, &ids, ArchiveRule::ByTag, ArchiveMode::Move, &dest).unwrap();
    let r = archive::execute(&conn, &p).unwrap();
    assert_eq!(r.count, 3);

    let undone = archive::undo(&conn, r.op_id).unwrap();
    assert_eq!(undone, 3);
    for item in &p.items {
        assert!(item.from_path.exists(), "原路径应恢复: {}", item.from_path.display());
        assert!(!item.to_path.exists(), "归档目标应清空");
        assert!(db::get_photo_by_path(&conn, &item.from_path).unwrap().is_some());
    }
    assert_eq!(db::count_photos(&conn).unwrap(), 3);
    assert!(db::list_ops(&conn, 10).unwrap().is_empty(), "撤销后删除该日志");

    cleanup(&paths, conn);
}

#[test]
fn conflict_appends_suffix_without_overwriting() {
    let (paths, conn, _photos, ids) = setup(1);
    let dest = dest_of(&paths);

    // 先算出目标路径并制造同名文件
    let first = archive::plan(&conn, &ids, ArchiveRule::ByYearMonth, ArchiveMode::Move, &dest).unwrap();
    let occupied = first.items[0].to_path.clone();
    fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    fs::write(&occupied, b"existing").unwrap();

    let p = archive::plan(&conn, &ids, ArchiveRule::ByYearMonth, ArchiveMode::Move, &dest).unwrap();
    assert_eq!(p.conflict_count, 1);
    assert!(p.items[0].conflict);
    assert_ne!(p.items[0].to_path, occupied);

    archive::execute(&conn, &p).unwrap();
    assert_eq!(fs::read(&occupied).unwrap(), b"existing", "既有文件不得被覆盖");
    assert!(p.items[0].to_path.exists());
    assert!(p.items[0].to_path.to_string_lossy().contains("_1"));

    archive::undo(&conn, db::list_ops(&conn, 1).unwrap()[0].id).unwrap();
    assert!(first.items[0].from_path.exists());
    assert_eq!(fs::read(&occupied).unwrap(), b"existing");

    cleanup(&paths, conn);
}

#[test]
fn four_rules_and_missing_metadata_fallback() {
    let (paths, conn, _photos, ids) = setup(1);
    let dest = dest_of(&paths);

    // 清空元数据，验证兜底目录名
    conn.execute(
        "UPDATE photos SET date_taken=NULL, tags='', location=NULL WHERE id=?1",
        params![ids[0]],
    )
    .unwrap();
    let cases: [(ArchiveRule, Vec<&str>); 4] = [
        (ArchiveRule::ByYear, vec!["未知日期"]),
        (ArchiveRule::ByYearMonth, vec!["未知日期", "未知日期"]),
        (ArchiveRule::ByStory, vec!["未知日期", "未知日期_未命名故事"]),
        (ArchiveRule::ByTag, vec!["未分类"]),
    ];
    for (rule, expected) in cases {
        let p = archive::plan(&conn, &ids, rule, ArchiveMode::Move, &dest).unwrap();
        assert_eq!(p.items.len(), 1);
        let parts: Vec<&str> = p.items[0].relative.split('/').collect();
        let dirs = &parts[..parts.len() - 1];
        assert_eq!(dirs, expected, "规则 {:?} 的目录层级", rule);
    }

    // 正常元数据：故事规则含年/月与地点，标签规则取第一个标签
    conn.execute(
        "UPDATE photos SET date_taken='2024-08-15 09:45:00', tags='旅行,风景', location='富士山' WHERE id=?1",
        params![ids[0]],
    )
    .unwrap();
    let story = archive::plan(&conn, &ids, ArchiveRule::ByStory, ArchiveMode::Move, &dest).unwrap();
    assert!(story.items[0].relative.starts_with("2024/2024-08_富士山/"));
    let tag = archive::plan(&conn, &ids, ArchiveRule::ByTag, ArchiveMode::Move, &dest).unwrap();
    assert!(tag.items[0].relative.starts_with("旅行/"));

    cleanup(&paths, conn);
}

#[test]
fn plan_is_read_only_and_settings_roundtrip() {
    let (paths, conn, _photos, ids) = setup(2);
    let dest = dest_of(&paths);

    let p = archive::plan(&conn, &ids, ArchiveRule::ByYearMonth, ArchiveMode::Move, &dest).unwrap();
    assert_eq!(p.items.len(), 2);
    assert!(!dest.exists(), "预览不得创建归档目录或移动任何文件");
    for item in &p.items {
        assert!(item.from_path.exists(), "预览后源文件仍在");
    }
    assert!(db::list_ops(&conn, 10).unwrap().is_empty(), "预览不得写操作日志");

    db::set_setting(&conn, "archive.rule", "tag").unwrap();
    db::set_setting(&conn, "archive.mode", "copy").unwrap();
    db::set_setting(&conn, "archive.mode", "move").unwrap();
    assert_eq!(db::get_setting(&conn, "archive.rule").unwrap().as_deref(), Some("tag"));
    assert_eq!(db::get_setting(&conn, "archive.mode").unwrap().as_deref(), Some("move"));
    assert_eq!(db::get_setting(&conn, "not.exist").unwrap(), None);

    cleanup(&paths, conn);
}
