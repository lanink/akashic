//! 扫描器：递归遍历目录，提取元数据与内容哈希，生成缩略图缓存，增量写入索引。
//!
//! 不依赖 gpui，可在后台线程运行；进度通过 `ScanProgress` 回调上报。

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Instant;

use anyhow::{Context, Result};
use exif::{In, Reader as ExifReader, Tag};
use walkdir::WalkDir;

use crate::core::config::Paths;
use crate::core::db;
use crate::core::model::{Photo, ScanProgress, ScanStats};
use crate::core::thumbnail;

/// 支持的图片扩展名（小写比较）。
const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff", "heic", "heif",
];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_skippable_dir(name: &str) -> bool {
    name.starts_with('.') || name == "trash" || name == "node_modules" || name == "target"
}

/// 从文件读取 EXIF 拍摄时间与设备。
fn read_exif(path: &Path) -> (Option<String>, Option<String>) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None),
    };
    let mut reader = BufReader::new(file);
    let exif = match ExifReader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return (None, None),
    };

    let date = exif
        .get_field(Tag::DateTimeOriginal, In::PRIMARY)
        .and_then(|f| f.display_value().to_string().parse::<String>().ok())
        // display_value 形如 "2024:08:15 09:45:00"，转换为标准格式
        .map(|s| normalize_exif_date(&s));

    let make = exif.get_field(Tag::Make, In::PRIMARY).map(|f| f.display_value().to_string());
    let model = exif.get_field(Tag::Model, In::PRIMARY).map(|f| f.display_value().to_string());
    let device = match (make, model) {
        (Some(m), Some(md)) => Some(format!("{} {}", m.trim(), md.trim())),
        (Some(m), None) => Some(m.trim().to_string()),
        (None, Some(md)) => Some(md.trim().to_string()),
        (None, None) => None,
    };
    (date, device)
}

/// "2024:08:15 09:45:00" -> "2024-08-15 09:45:00"
fn normalize_exif_date(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 19 {
        let mut chars: Vec<char> = s.chars().collect();
        for i in [4, 7] {
            if i < chars.len() {
                chars[i] = '-';
            }
        }
        chars.into_iter().take(19).collect()
    } else {
        s.to_string()
    }
}

/// 计算文件内容哈希（Twox64，作为去重与缓存 key 的辅助）。
#[allow(deprecated)]
fn content_hash(path: &Path) -> String {
    use std::hash::Hasher;
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = twox_hash::XxHash64::with_seed(0);
    let mut buf = [0u8; 64 * 1024];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.write(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    format!("{:016x}", hasher.finish())
}

fn modified_millis(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 从路径派生展示用地点（取父目录名），本期无逆地理编码。
fn location_from_path(path: &Path) -> Option<String> {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// 扫描单个根目录，把结果写入索引。进度通过 `progress` 上报（可为 None）。
pub fn scan_root(
    paths: &Paths,
    conn: &rusqlite::Connection,
    root: &Path,
    progress: Option<&Sender<ScanProgress>>,
) -> Result<ScanStats> {
    let started = Instant::now();
    let root_id = db::upsert_scan_root(conn, root)?;
    if let Some(tx) = progress {
        let _ = tx.send(ScanProgress::Started { root: root.to_path_buf() });
    }

    let mut stats = ScanStats::default();
    let mut alive_paths: Vec<PathBuf> = Vec::new();

    // 先收集所有图片路径，避免边遍历边写库造成的生命周期复杂度。
    let image_paths: Vec<PathBuf> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            !e.file_type().is_dir() || !is_skippable_dir(&e.file_name().to_string_lossy())
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_image(e.path()))
        .map(|e| e.into_path())
        .collect();

    let tx = conn.unchecked_transaction().context("开启扫描事务失败")?;
    for image_path in &image_paths {
        stats.photos_found += 1;
        let meta = match std::fs::metadata(image_path) {
            Ok(m) => m,
            Err(_) => {
                stats.errors += 1;
                continue;
            }
        };

        let (date_taken, device) = read_exif(image_path);
        let date_taken = date_taken.or_else(|| {
            // 无 EXIF 时用文件修改时间格式化
            chrono::DateTime::from_timestamp_millis(modified_millis(&meta)).map(|dt| {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
        });

        let hash = content_hash(image_path);

        // 缩略图：以哈希命名，存在则跳过
        let thumb_path = thumbnail::ensure_thumbnail(paths, image_path, &hash).ok();

        // 读取尺寸（image crate 只解码头部，imageflow 留给 M2 精细处理）
        let (width, height) = image::image_dimensions(image_path).unwrap_or((0, 0));

        let photo = Photo {
            id: 0,
            path: image_path.clone(),
            file_name: image_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            size_bytes: meta.len() as i64,
            date_taken,
            modified_at: modified_millis(&meta),
            width: width as i32,
            height: height as i32,
            device,
            location: location_from_path(image_path),
            tags: String::new(),
            content_hash: hash,
            thumbnail_path: thumb_path,
            gps_lon_wgs84: None, // 本期暂缓 GPS
            gps_lat_wgs84: None,
        };

        match db::upsert_photo(&tx, &photo, root_id) {
            Ok(true) => stats.photos_added += 1,
            Ok(false) => stats.photos_skipped += 1,
            Err(_) => stats.errors += 1,
        }
        alive_paths.push(image_path.clone());

        if let Some(sender) = progress {
            let _ = sender.send(ScanProgress::Scanning {
                processed: stats.photos_found,
            });
        }
    }

    // 增量清理：删除该根目录下不再存在的记录
    match db::sweep_photos(&tx, root, &alive_paths) {
        Ok(removed) => stats.photos_removed = removed,
        Err(e) => {
            eprintln!("[akashic] 增量清理失败: {e}");
            stats.errors += 1;
        }
    }

    tx.commit().context("提交扫描事务失败")?;
    stats.duration_ms = started.elapsed().as_millis();

    if let Some(sender) = progress {
        let _ = sender.send(ScanProgress::Finished { stats: stats.clone() });
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_exif_date() {
        assert_eq!(normalize_exif_date("2024:08:15 09:45:00"), "2024-08-15 09:45:00");
        assert_eq!(normalize_exif_date("2024:01:02 03:04:05"), "2024-01-02 03:04:05");
        assert_eq!(normalize_exif_date("bad"), "bad");
    }

    #[test]
    fn test_is_image_and_skip_dirs() {
        assert!(is_image(Path::new("a.JPG")));
        assert!(is_image(Path::new("a.jpeg")));
        assert!(is_image(Path::new("a.webp")));
        assert!(!is_image(Path::new("a.txt")));
        assert!(is_skippable_dir(".hidden"));
        assert!(is_skippable_dir("trash"));
        assert!(!is_skippable_dir("日本旅行"));
    }
}
