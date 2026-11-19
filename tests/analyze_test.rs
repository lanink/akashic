//! 分析集成测试：在临时目录生成清晰/模糊/重复/截图样本，
//! 验证 扫描 → 分析 → 落库 → 查询 的完整链路。绝不触碰真实相册。

use std::fs;
use std::path::PathBuf;

use akashic::core::analyze::blur::BLUR_THRESHOLD;
use akashic::core::analyze::{AnalyzeProgress, analyze_library};
use akashic::core::config::Paths;
use akashic::core::{db, scanner};
use image::{ImageBuffer, Luma, Rgb, RgbImage};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("akashic_analyze_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 带中等频率条纹的清晰图（降采样后仍保留边缘，模拟真实照片细节）。
fn make_sharp(path: &PathBuf) {
    let img = RgbImage::from_fn(200, 200, |x, y| {
        let v = if (x / 8) % 2 == 0 { 255 } else { 0 };
        let _ = y;
        Rgb([v, v, v])
    });
    img.save(path).unwrap();
}

/// 纯色图（模糊图：无高频细节）。
fn make_blurred(path: &PathBuf) {
    let img: ImageBuffer<Luma<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(200, 200, Luma([128]));
    img.save(path).unwrap();
}

#[test]
fn analyze_index_end_to_end() {
    let base = temp_dir("e2e");
    let photos_dir = base.join("photos");
    fs::create_dir_all(&photos_dir).unwrap();

    // 两张内容相同的清晰图（重复组）+ 一张纯色模糊图 + 一张截图命名的图
    make_sharp(&photos_dir.join("DSC_0001.jpg"));
    make_sharp(&photos_dir.join("DSC_0002.jpg"));
    make_blurred(&photos_dir.join("blurry.jpg"));
    make_blurred(&photos_dir.join("Screenshot_2024.png"));

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();

    let stats = scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    assert_eq!(stats.photos_found, 4);

    // 收集进度事件
    let (tx, rx) = std::sync::mpsc::channel::<AnalyzeProgress>();
    let report = analyze_library(&conn, Some(&tx)).unwrap();
    assert_eq!(report.photos_analyzed, 4, "4 张都应成功分析");
    assert_eq!(report.photos_failed, 0);

    let events: Vec<_> = rx.try_iter().collect();
    assert!(matches!(events.first(), Some(AnalyzeProgress::Started { total: 4 })));
    assert!(matches!(events.last(), Some(AnalyzeProgress::Finished { .. })));

    // 模糊检测：纯色图分数应低于阈值，清晰条纹图应高于阈值
    let all = db::list_photos(&conn).unwrap();
    let score = |name: &str| all.iter().find(|p| p.file_name == name).unwrap().blur_score.unwrap();
    let blurry_score = score("blurry.jpg");
    let sharp_score = score("DSC_0001.jpg");
    assert!(blurry_score < BLUR_THRESHOLD, "纯色图应低于模糊阈值: {blurry_score}");
    assert!(sharp_score > BLUR_THRESHOLD, "清晰图应高于模糊阈值: {sharp_score}");

    // list_blurry 按分数升序，最模糊的应排在前面
    let blurry = db::list_blurry(&conn, 4).unwrap();
    assert_eq!(blurry[0].file_name, "blurry.jpg", "最模糊的图应排第一");
    assert_eq!(blurry[1].file_name, "Screenshot_2024.png", "纯色截图应排第二");

    // 截图识别：Screenshot_ 命名应命中
    let screenshots = db::list_screenshots(&conn).unwrap();
    assert!(
        screenshots.iter().any(|p| p.file_name == "Screenshot_2024.png"),
        "截图命名的图应被识别"
    );

    // 大文件：应返回按大小排序的结果（这里 4 张，取 2 张）
    let large = db::list_large_files(&conn, 2).unwrap();
    assert_eq!(large.len(), 2);
    assert!(large[0].size_bytes >= large[1].size_bytes);

    // 相似分组：两张内容相同的清晰图应归为一组
    let groups = db::list_photo_groups(&conn).unwrap();
    assert!(
        groups.iter().any(|g| g.members.len() >= 2
            && g.members.iter().all(|m| m.file_name.starts_with("DSC_"))),
        "两张相同的清晰图应归为同一相似组: {:?}",
        groups.iter().map(|g| g.members.iter().map(|m| m.file_name.clone()).collect::<Vec<_>>()).collect::<Vec<_>>()
    );

    assert_eq!(db::count_analyzed(&conn).unwrap(), 4);

    drop(conn);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn rerun_analysis_is_idempotent() {
    let base = temp_dir("rerun");
    let photos_dir = base.join("photos");
    fs::create_dir_all(&photos_dir).unwrap();
    make_sharp(&photos_dir.join("a.jpg"));
    make_sharp(&photos_dir.join("b.jpg"));

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();

    let r1 = analyze_library(&conn, None).unwrap();
    let g1 = db::list_photo_groups(&conn).unwrap().len();
    let r2 = analyze_library(&conn, None).unwrap();
    let g2 = db::list_photo_groups(&conn).unwrap().len();

    // 重跑不重复累积分组
    assert_eq!(g1, g2, "重复分析不应累积相似组");
    assert_eq!(r1.photos_analyzed, r2.photos_analyzed);

    drop(conn);
    let _ = fs::remove_dir_all(&base);
}

/// 回归：坏图（解码失败）绝不能被写成 blur_score = 0.0 并计入「已分析 / 模糊列表」。
#[test]
fn corrupt_image_is_not_recorded_as_zero_blur() {
    let base = temp_dir("corrupt");
    let photos_dir = base.join("photos");
    fs::create_dir_all(&photos_dir).unwrap();
    let good = photos_dir.join("good.jpg");
    make_sharp(&good);
    // 放在扫描根之外，不会被索引；用来模拟索引指向一个已损坏的文件。
    let broken = base.join("broken.jpg");
    fs::write(&broken, b"this is definitely not a jpeg").unwrap();

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();
    scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    let id = db::list_photos(&conn).unwrap()[0].id;
    conn.execute(
        "UPDATE photos SET path=?1, file_name='broken.jpg', thumbnail_path=NULL WHERE id=?2",
        rusqlite::params![broken.to_string_lossy().to_string(), id],
    )
    .unwrap();

    let photo = db::get_photo_by_id(&conn, id).unwrap().unwrap();
    assert!(
        akashic::core::analyze::analyze_photo(&photo).is_none(),
        "坏图不得产出分析结果"
    );

    let report = analyze_library(&conn, None).unwrap();
    assert_eq!(report.photos_failed, 1);
    assert_eq!(report.photos_analyzed, 0);
    let after = db::get_photo_by_id(&conn, id).unwrap().unwrap();
    assert_eq!(after.blur_score, None, "坏图不得被写成 0.0");
    assert_eq!(db::count_analyzed(&conn).unwrap(), 0, "坏图不得计入已分析");
    assert!(db::list_blurry(&conn, 10).unwrap().is_empty(), "坏图不得出现在模糊列表");

    drop(conn);
    let _ = fs::remove_dir_all(&base);
}

/// 回归：纯色/低纹理图没有可比较的结构特征，pHash 应返回 None，
/// 否则所有纯色图会得到同一个伪哈希并被判成巨量相似组。
#[test]
fn flat_color_image_has_no_phash() {
    let flat = image::GrayImage::from_pixel(64, 64, image::Luma([128]));
    assert_eq!(
        akashic::core::analyze::similarity::phash_from_gray(&flat),
        None,
        "纯色图不得得到伪哈希"
    );
    let striped = image::GrayImage::from_fn(64, 64, |x, _| {
        image::Luma([if (x / 8) % 2 == 0 { 255 } else { 0 }])
    });
    assert!(
        akashic::core::analyze::similarity::phash_from_gray(&striped).is_some(),
        "有纹理的图应能算出哈希"
    );
}

/// 回归：代表元聚类不得把 A~B、B~C 但 A≁C 串成一个巨组
/// （旧并查集传递闭包会把所有链式相近的图并成一组）。
#[test]
fn representative_clustering_does_not_chain_transitively() {
    let h = akashic::core::analyze::similarity::hamming;
    let group = akashic::core::analyze::similarity::group_by_phash;
    let a: u64 = 0;
    let b: u64 = 0b0000_1111;
    let c: u64 = 0b1111_1111;
    assert_eq!(h(a, b), 4);
    assert_eq!(h(b, c), 4);
    assert_eq!(h(a, c), 8);

    let groups = group(&[(1, a), (2, b), (3, c)], 4);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0], vec![1, 2]);
    assert!(!groups.iter().any(|g| g.len() >= 3), "不得产生 3 人巨组: {groups:?}");
}
