//! M6 性能基准：5000 张照片场景下的扫描 / 冷启动加载 / 筛选耗时。
//!
//! 默认 `#[ignore]`，不拖慢日常 `cargo test`；显式运行：
//!   cargo test --release --test perf_test -- --ignored --nocapture
//! 或（debug 数字偏大，仅作相对参考）：
//!   cargo test --test perf_test -- --ignored --nocapture
//! 可用 `AKASHIC_BENCH_COUNT=500` 临时缩小规模做冒烟。
//! 只在临时目录生成合成图片，绝不触碰真实相册。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use akashic::core::config::Paths;
use akashic::core::{db, scanner};
use akashic::ui::library::LibraryData;
use image::{Rgb, RgbImage};

/// 目标场景规模：PRD 的目标用户「5000+ 张照片」。
const PHOTO_COUNT: usize = 5000;
/// 每个子目录的照片数，模拟真实相册的目录结构（同时测目录树派生）。
const PER_DIR: usize = 250;
/// 筛选耗时用多次调用取平均，避免单次抖动。
const FILTER_ROUNDS: usize = 100;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("akashic_bench_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成一张 16x16 纯色 PNG；颜色随序号变化，保证内容哈希互不相同，
/// 使缩略图缓存不会命中同一 key（否则基准会失真）。
fn make_png(path: &Path, i: usize) {
    let r = (i % 256) as u8;
    let g = ((i / 256) % 256) as u8;
    let b = ((i / 65536) % 256) as u8;
    let img = RgbImage::from_fn(16, 16, |_, _| Rgb([r, g, b]));
    img.save(path).unwrap();
}

#[test]
#[ignore = "性能基准：cargo test --release --test perf_test -- --ignored --nocapture"]
fn bench_5000_photos() {
    let count: usize = std::env::var("AKASHIC_BENCH_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(PHOTO_COUNT)
        .max(1); // 0 会导致除零与空目录树断言失败

    let base = temp_dir("5000");
    let photos_dir = base.join("photos");
    let dir_count = count.div_ceil(PER_DIR);
    for d in 0..dir_count {
        fs::create_dir_all(photos_dir.join(format!("roll_{:03}", d))).unwrap();
    }

    let prep_start = Instant::now();
    for i in 0..count {
        let dir = photos_dir.join(format!("roll_{:03}", i / PER_DIR));
        make_png(&dir.join(format!("IMG_{:05}.png", i)), i);
    }
    let prep = prep_start.elapsed();

    let paths = Paths::from_app_dir(base.join("appdata"));
    paths.ensure_dirs().unwrap();
    let conn = db::open(&paths.db_path).unwrap();

    // 1) 扫描：递归遍历 + EXIF + 内容哈希 + 缩略图 + SQLite 写入
    let scan_start = Instant::now();
    let stats = scanner::scan_root(&paths, &conn, &photos_dir, None).unwrap();
    let scan = scan_start.elapsed();

    // 2) 冷启动：重新加载索引快照 + 派生目录树
    let reload_start = Instant::now();
    let data = LibraryData::reload(&conn).unwrap();
    let reload = reload_start.elapsed();

    // 3) 筛选：visible_photos() 对全量索引做目录/关键词过滤
    let filter_start = Instant::now();
    let mut sink = 0usize;
    for _ in 0..FILTER_ROUNDS {
        sink += data.visible_photos().len();
    }
    let filter = filter_start.elapsed();

    let indexed = db::count_photos(&conn).unwrap();
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };

    println!("=== Akashic M6 perf bench ({count} photos, {profile}) ===");
    println!("prepare files      : {:.2?}", prep);
    println!("scan_root          : {:.2?}  ({:.2} ms/photo)", scan, scan.as_secs_f64() * 1000.0 / count as f64);
    println!("reload (cold start): {:.2?}", reload);
    println!(
        "visible_photos x{FILTER_ROUNDS} : {:.2?}  ({:.3} ms/call)",
        filter,
        filter.as_secs_f64() * 1000.0 / FILTER_ROUNDS as f64
    );
    println!("indexed            : {indexed} (found {})", stats.photos_found);
    assert_eq!(sink, count * FILTER_ROUNDS, "筛选结果数量应等于总数");
    assert_eq!(indexed, count as i64, "索引应包含全部照片");
    assert!(!data.folders.is_empty(), "应派生出目录树");

    // Windows 上需先释放 SQLite 连接才能删除临时目录
    drop(data);
    drop(conn);
    let _ = fs::remove_dir_all(&base);
}
