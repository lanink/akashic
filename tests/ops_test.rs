//! 批量操作与回收站集成测试：临时目录生成图片 + 扫描建库，验证
//! 批量重命名 100 张 + 一次撤销、删除进回收站 + 恢复、清空、冲突处理、操作日志。
//! 绝不触碰真实相册。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use akashic::core::config::Paths;
use akashic::core::ops::{rename, trash};
use akashic::core::{db, scanner};
use akashic::core::model::OpKind;
use image::{Rgb, RgbImage};

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(name: &str) -> PathBuf {
    // 每个用例唯一序号，避免并行测试共用临时目录（否则 database locked / 文件互删）
    let uniq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "akashic_ops_{}_{}_{}",
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

/// 生成 n 张图片并扫描入索引，返回 (paths, conn, 源目录, 照片 id 列表)。
fn setup(n: usize) -> (Paths, rusqlite::Connection, PathBuf, Vec<i64>) {
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
    let ids = db::list_photos(&conn).unwrap().into_iter().map(|p| p.id).collect();
    (paths, conn, photos, ids)
}

#[test]
fn rename_100_then_undo_once() {
    let (paths, conn, photos, ids) = setup(100);
    assert_eq!(ids.len(), 100);

    // 预览：只给 5 条样例，总数 100
    let pv = rename::preview(&conn, &ids, "{前缀}_旅行_{日期}_{序号:3}", "日本", 5).unwrap();
    assert_eq!(pv.total, 100);
    assert_eq!(pv.items.len(), 5);
    assert!(pv.items[0].new_name.ends_with(".jpg"));
    assert!(pv.items[0].new_name.starts_with("日本_旅行_"), "占位符应被替换");

    // 执行批量重命名
    let r = rename::execute(&conn, &ids, "{前缀}_旅行_{序号:3}", "日本").unwrap();
    assert_eq!(r.count, 100);
    assert_eq!(r.samples.len(), 5);
    assert_eq!(db::count_photos(&conn).unwrap(), 100, "记录数不变");

    // 磁盘上确实按新名存在，旧名消失
    assert!(photos.join("日本_旅行_001.jpg").exists());
    assert!(photos.join("日本_旅行_100.jpg").exists());
    assert!(!photos.join("IMG_0001.jpg").exists());

    // 操作日志含 Rename 一条，item_count=100
    let ops = db::list_ops(&conn, 10).unwrap();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, OpKind::Rename);
    assert_eq!(ops[0].item_count, 100);

    // 一次撤销：全部恢复原名
    let undone = rename::undo(&conn, r.op_id).unwrap();
    assert_eq!(undone, 100);
    assert!(photos.join("IMG_0001.jpg").exists());
    assert!(!photos.join("日本_旅行_001.jpg").exists());
    assert_eq!(db::list_ops(&conn, 10).unwrap().len(), 0, "撤销后删除该日志");
    assert_eq!(db::search_photos(&conn, "IMG_0001").unwrap().len(), 1);

    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());
}

#[test]
fn rename_conflict_appends_suffix() {
    let (paths, conn, photos, mut ids) = setup(2);
    // 制造冲突：预先放一个同名文件
    fs::write(photos.join("固定名.jpg"), b"existing").unwrap();
    ids.truncate(1); // 只重命名一张

    let r = rename::execute(&conn, &ids, "固定名", "").unwrap();
    assert_eq!(r.count, 1);
    assert_eq!(r.samples[0].new_name, "固定名_1.jpg", "冲突应追加 _1 后缀");
    assert!(photos.join("固定名_1.jpg").exists());
    // 原冲突文件未被破坏
    assert_eq!(fs::read(photos.join("固定名.jpg")).unwrap(), b"existing");

    rename::undo(&conn, r.op_id).unwrap();
    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());
}

#[test]
fn trash_and_restore_preserves_files() {
    let (paths, conn, photos, ids) = setup(3);
    let target = ids[0];
    let target_path = db::get_photo_by_id(&conn, target).unwrap().unwrap().path.clone();

    // 删除（移动）到回收站
    let tr = trash::trash_photos(&paths, &conn, &[target]).unwrap();
    assert_eq!(tr.count, 1);
    assert!(tr.bytes > 0);
    assert!(!target_path.exists(), "原路径文件应已移走");
    assert_eq!(db::count_photos(&conn).unwrap(), 2, "索引记录应移除");

    // 回收站有 1 项，文件确实在回收站目录内
    let (n, _) = db::trash_summary(&conn).unwrap();
    assert_eq!(n, 1);
    let titems = db::list_trash(&conn).unwrap();
    assert_eq!(titems.len(), 1);
    assert!(titems[0].trash_path.exists());

    // 恢复：文件回到原路径，索引记录重建
    let restored = trash::restore_items(&paths, &conn, &[titems[0].id]).unwrap();
    assert_eq!(restored, 1);
    assert!(target_path.exists(), "原路径文件应恢复");
    assert_eq!(db::count_photos(&conn).unwrap(), 3);
    assert_eq!(db::trash_summary(&conn).unwrap().0, 0);
    assert!(db::search_photos(&conn, "IMG_0001").unwrap().len() >= 1);

    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());
    let _ = photos;
}

#[test]
fn trash_undo_and_empty_trash() {
    // 撤销删除
    let (paths, conn, _photos, ids) = setup(2);
    let tr = trash::trash_photos(&paths, &conn, &ids).unwrap();
    assert_eq!(tr.count, 2);
    let undone = trash::undo_trash(&paths, &conn, tr.op_id).unwrap();
    assert_eq!(undone, 2);
    assert_eq!(db::count_photos(&conn).unwrap(), 2);
    assert_eq!(db::trash_summary(&conn).unwrap().0, 0);
    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());

    // 清空回收站：永久删除的是回收站副本，用户原路径已不在
    let (paths2, conn2, photos2, ids2) = setup(2);
    trash::trash_photos(&paths2, &conn2, &ids2).unwrap();
    let titems = db::list_trash(&conn2).unwrap();
    let n = trash::empty_trash(&paths2, &conn2).unwrap();
    assert_eq!(n, 2);
    assert_eq!(db::trash_summary(&conn2).unwrap().0, 0);
    for it in &titems {
        assert!(!it.trash_path.exists(), "回收站副本应被永久删除");
    }
    assert!(!photos2.join("IMG_0001.jpg").exists());
    drop(conn2);
    let _ = fs::remove_dir_all(paths2.data_dir.parent().unwrap());
}

#[test]
fn purge_expired_removes_old_items_only() {
    let (paths, conn, _photos, ids) = setup(2);
    trash::trash_photos(&paths, &conn, &ids).unwrap();
    // 把一项的 trashed_at 改成 40 天前（模拟过期）
    let now = db::now_millis();
    let old = now - 40 * 24 * 60 * 60 * 1000;
    conn.execute("UPDATE trash_items SET trashed_at=?1 WHERE id=(SELECT MIN(id) FROM trash_items)",
        rusqlite::params![old]).unwrap();

    let purged = trash::purge_expired(&conn, 30).unwrap();
    assert_eq!(purged, 1, "仅清理超过 30 天的项目");
    assert_eq!(db::trash_summary(&conn).unwrap().0, 1);

    let ops = db::list_ops(&conn, 10).unwrap();
    assert!(ops.iter().any(|o| o.kind == OpKind::Purge));

    drop(conn);
    let _ = fs::remove_dir_all(paths.data_dir.parent().unwrap());
}
