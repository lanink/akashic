//! 扫描器集成测试：在临时目录生成图片，验证扫描 / 增量 / 过滤 / 搜索。
//! 绝不触碰真实相册。

use std::fs;
use std::path::PathBuf;

use akashic::core::config::Paths;
use akashic::core::{db, scanner};
use image::{Rgb, RgbImage};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("akashic_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_jpeg(path: &PathBuf, r: u8, g: u8, b: u8) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let img = RgbImage::from_fn(20, 20, |_, _| Rgb([r, g, b]));
    img.save(path).unwrap();
}

#[test]
fn scan_into_index_and_incremental() {
    let base = temp_dir("scan");
    let photos_dir = base.join("photos");
    let trip = photos_dir.join("trip");
    make_jpeg(&trip.join("IMG_0001.jpg"), 255, 0, 0);
    make_jpeg(&trip.join("IMG_0002.png"), 0, 255, 0);
    make_jpeg(&photos_dir.join("root.jpg"), 0, 0, 255);
    // 放一个非图片文件，应被忽略
    fs::write(trip.join("notes.txt"), "not an image").unwrap();

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();

    let stats = scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    assert_eq!(stats.photos_found, 3, "应找到 3 张图片（忽略 txt）");
    assert_eq!(db::count_photos(&conn).unwrap(), 3);

    // 重复扫描：跳跃未变化，不产生重复记录
    let stats2 = scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    assert_eq!(stats2.photos_skipped, 3, "未变化的照片应被跳过");
    assert_eq!(db::count_photos(&conn).unwrap(), 3, "重复扫描不应产生重复记录");

    // 地点派生：trip 目录下的照片 location = "trip"
    let trip_photo = db::get_photo_by_path(&conn, &trip.join("IMG_0001.jpg"))
        .unwrap()
        .unwrap();
    assert_eq!(trip_photo.location.as_deref(), Some("trip"));
    assert_eq!(trip_photo.width, 20);
    assert_eq!(trip_photo.height, 20);
    assert!(!trip_photo.content_hash.is_empty(), "应计算内容哈希");
    assert!(trip_photo.thumbnail_path.is_some(), "应生成缩略图");

    // 目录过滤
    assert_eq!(db::list_photos_under(&conn, &trip).unwrap().len(), 2);
    // 搜索
    assert_eq!(db::search_photos(&conn, "IMG_0001").unwrap().len(), 1);
    assert_eq!(db::search_photos(&conn, "trip").unwrap().len(), 2);

    // 增量清理：删除一张源图片后重扫，索引应同步移除
    fs::remove_file(trip.join("IMG_0002.png")).unwrap();
    let stats3 = scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    assert_eq!(stats3.photos_removed, 1, "应清理 1 条失效记录");
    assert_eq!(db::count_photos(&conn).unwrap(), 2);

    // Windows 上需先释放 SQLite 连接才能删除临时目录
    drop(conn);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn thumbnails_are_reused() {
    let base = temp_dir("thumb");
    let src = base.join("photo.jpg");
    make_jpeg(&src, 128, 64, 32);

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let hash = "abcd1234abcd1234";
    let t1 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src, hash).unwrap();
    assert!(t1.exists());
    let modified1 = fs::metadata(&t1).unwrap().modified().unwrap();

    // 第二次调用直接复用，不重写文件
    let t2 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src, hash).unwrap();
    assert_eq!(t1, t2);
    let modified2 = fs::metadata(&t2).unwrap().modified().unwrap();
    assert_eq!(modified1, modified2, "已存在的缩略图不应被重写");

    fs::remove_dir_all(&base).unwrap();
}

/// 回归：目录名里的 `_` / `%` 是 LIKE 通配符，未转义会越界匹配到别的目录。
#[test]
fn like_wildcards_in_directory_name_are_escaped() {
    let base = temp_dir("like");
    let photos = base.join("photos");
    let a = photos.join("2024_旅行");
    let b = photos.join("2024X旅行");
    let c = photos.join("100%真实");
    let d = photos.join("100ABC真实");
    make_jpeg(&a.join("a.jpg"), 1, 2, 3);
    make_jpeg(&b.join("b.jpg"), 4, 5, 6);
    make_jpeg(&c.join("c.jpg"), 7, 8, 9);
    make_jpeg(&d.join("d.jpg"), 10, 11, 12);

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    scanner::scan_root(&paths, &conn, &photos, None).unwrap();
    assert_eq!(db::count_photos(&conn).unwrap(), 4);

    let under = db::list_photos_under(&conn, &a).unwrap();
    assert_eq!(under.len(), 1, "目录名里的 _ 不得被 LIKE 当通配符");
    assert_eq!(under[0].file_name, "a.jpg");

    let under_pct = db::list_photos_under(&conn, &c).unwrap();
    assert_eq!(under_pct.len(), 1, "目录名里的 % 不得被 LIKE 当通配符");
    assert_eq!(under_pct[0].file_name, "c.jpg");

    drop(conn);
    fs::remove_dir_all(&base).unwrap();
}

/// 回归：增量清理按 scan_root_id 精确作用，扫描第二个根不得清空第一个根的索引。
#[test]
fn sweep_photos_is_scoped_to_scan_root() {
    let base = temp_dir("sweep");
    let root1 = base.join("root1");
    let root2 = base.join("root2");
    make_jpeg(&root1.join("a.jpg"), 1, 2, 3);
    make_jpeg(&root2.join("b.jpg"), 4, 5, 6);

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    scanner::scan_root(&paths, &conn, &root1, None).unwrap();
    scanner::scan_root(&paths, &conn, &root2, None).unwrap();
    assert_eq!(db::count_photos(&conn).unwrap(), 2, "扫描第二个根不得清空第一个根");

    let r1 = db::upsert_scan_root(&conn, &root1).unwrap();
    assert_eq!(db::sweep_photos(&conn, r1, &[root1.join("a.jpg")]).unwrap(), 0);
    assert_eq!(db::count_photos(&conn).unwrap(), 2);

    assert_eq!(db::sweep_photos(&conn, r1, &[]).unwrap(), 1);
    assert_eq!(db::count_photos(&conn).unwrap(), 1, "root2 的记录必须保留");
    assert!(db::get_photo_by_path(&conn, &root2.join("b.jpg")).unwrap().is_some());

    drop(conn);
    fs::remove_dir_all(&base).unwrap();
}

/// 回归：0 字节缓存是上次写入崩溃留下的残片，必须重新生成而不是直接复用。
#[test]
fn zero_byte_thumbnail_cache_is_regenerated() {
    let base = temp_dir("thumb_zero");
    let src = base.join("photo.jpg");
    make_jpeg(&src, 10, 20, 30);
    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();

    let t1 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src, "zero-hash").unwrap();
    fs::write(&t1, b"").unwrap();
    assert_eq!(fs::metadata(&t1).unwrap().len(), 0);
    let t2 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src, "zero-hash").unwrap();
    assert_eq!(t1, t2);
    assert!(fs::metadata(&t2).unwrap().len() > 0, "0 字节缓存必须被重建");

    fs::remove_dir_all(&base).unwrap();
}

/// 回归：缓存目录丢失要能重建；哈希退化 key 要加 fb_ 前缀且与正常 key 命名空间隔离。
#[test]
fn thumbnail_recreates_cache_dir_and_namespaces_fallback() {
    let base = temp_dir("thumb_dir");
    let src1 = base.join("photo1.jpg");
    let src2 = base.join("photo2.jpg");
    make_jpeg(&src1, 10, 20, 30);
    make_jpeg(&src2, 30, 20, 10);
    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();

    let t = akashic::core::thumbnail::ensure_thumbnail(&paths, &src1, "normal-hash").unwrap();
    assert!(t.exists());
    fs::remove_dir_all(&paths.cache_dir).unwrap();
    let t2 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src1, "normal-hash").unwrap();
    assert!(t2.exists() && fs::metadata(&t2).unwrap().len() > 0, "缓存目录应被重建");

    let f1 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src1, "").unwrap();
    let f2 = akashic::core::thumbnail::ensure_thumbnail(&paths, &src2, "").unwrap();
    let n1 = f1.file_name().unwrap().to_string_lossy().to_string();
    assert!(n1.starts_with("fb_"), "退化 key 需加 fb_ 前缀: {n1}");
    assert_ne!(f1, f2, "不同来源的退化 key 不得互相覆盖缓存");
    assert!(!n1.contains("normal-hash"), "退化 key 与正常内容哈希命名空间应隔离");

    fs::remove_dir_all(&base).unwrap();
}
