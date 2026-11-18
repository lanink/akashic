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
