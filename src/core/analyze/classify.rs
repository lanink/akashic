//! 分类筛选：截图启发式识别 + 大文件排序。
//!
//! 不依赖 gpui，可独立单元测试。函数直接消费 `Photo` 引用，
//! 由上层 `analyze_library` 汇总结果并落库。

use crate::core::model::Photo;

/// 截图判定分数阈值（0..100）。
pub const SCREENSHOT_THRESHOLD: u32 = 50;

/// 文件名中的强截图关键词（命中即高分）。
const STRONG_NAME_MARKERS: &[&str] = &[
    "screenshot", "screen shot", "screen-shot", "screenshot_",
    "截屏", "截图", "屏幕快照", "screenrecord",
];

/// 文件名中的弱截图关键词。
const WEAK_NAME_MARKERS: &[&str] = &["screen", "capture", "snap", "shot"];

/// 常见手机/屏幕的宽高比（高/宽 或 宽/高），用于辅助判断截图。
const SCREEN_RATIOS: &[f64] = &[
    16.0 / 9.0,   // 横屏
    9.0 / 16.0,   // 竖屏
    19.5 / 9.0,   // 全面屏
    9.0 / 19.5,
    4.0 / 3.0,
    3.0 / 4.0,
];

/// 计算一张照片是截图的可能性（0..100）。
///
/// 规则来自启发式，非精确判定：文件名关键词权重最高，
/// 缺少相机 EXIF、常见屏幕比例、标签命中作为辅助证据。
pub fn screenshot_likelihood(photo: &Photo) -> u32 {
    let mut score = 0u32;
    let name_lower = photo.file_name.to_lowercase();

    if STRONG_NAME_MARKERS.iter().any(|m| name_lower.contains(m)) {
        score += 60;
    } else if WEAK_NAME_MARKERS.iter().any(|m| name_lower.contains(m)) {
        score += 35;
    }

    // 截图通常没有相机设备 EXIF 信息
    if photo.device.as_deref().unwrap_or("").is_empty() {
        score += 20;
    }

    // 宽高比接近常见屏幕比例
    if photo.width > 0 && photo.height > 0 {
        let ratio = photo.height as f64 / photo.width as f64;
        if SCREEN_RATIOS.iter().any(|r| (ratio - r).abs() < 0.03) {
            score += 15;
        }
    }

    // 标签命中
    let tags = photo.tags.to_lowercase();
    if tags.contains("截图") || tags.contains("screenshot") {
        score += 25;
    }

    score.min(100)
}

/// 是否判定为截图。
pub fn is_screenshot(photo: &Photo) -> bool {
    screenshot_likelihood(photo) >= SCREENSHOT_THRESHOLD
}

/// 从照片列表中筛选所有截图。
pub fn find_screenshots(photos: &[Photo]) -> Vec<&Photo> {
    photos.iter().filter(|p| is_screenshot(p)).collect()
}

/// 按文件大小降序返回前 `top_n` 张大文件。
pub fn find_large_files(photos: &[Photo], top_n: usize) -> Vec<&Photo> {
    let mut sorted: Vec<&Photo> = photos.iter().collect();
    sorted.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    sorted.into_iter().take(top_n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn photo(name: &str, device: Option<&str>, w: i32, h: i32, size: i64) -> Photo {
        Photo {
            id: 0,
            path: PathBuf::from(format!("/tmp/{name}")),
            file_name: name.to_string(),
            size_bytes: size,
            date_taken: None,
            modified_at: 0,
            width: w,
            height: h,
            device: device.map(|s| s.to_string()),
            location: None,
            tags: String::new(),
            content_hash: String::new(),
            thumbnail_path: None,
            gps_lon_wgs84: None,
            gps_lat_wgs84: None,
            blur_score: None,
            is_screenshot: false,
            phash: None,
            group_id: None,
        }
    }

    #[test]
    fn strong_filename_marker_is_screenshot() {
        let p = photo("Screenshot_20240815.jpg", None, 1080, 1920, 1000);
        assert!(is_screenshot(&p));
    }

    #[test]
    fn chinese_filename_marker_is_screenshot() {
        let p = photo("截屏2024-08-15.png", None, 1920, 1080, 1000);
        assert!(is_screenshot(&p));
    }

    #[test]
    fn camera_photo_with_device_is_not_screenshot() {
        // 有设备 EXIF + 普通相机比例 + 无截图关键词 -> 20(无device不成立) 只有比例15
        let p = photo("IMG_1234.jpg", Some("Apple iPhone 15"), 4032, 3024, 4_000_000);
        assert!(!is_screenshot(&p));
    }

    #[test]
    fn weak_markers_alone_are_not_enough() {
        // 有 device + 非屏幕比例尺寸时，弱关键词单独仅 35 分，不足阈值
        let p = photo("screen_capture.png", Some("SomeCam"), 777, 531, 1000);
        assert_eq!(screenshot_likelihood(&p), 35);
        assert!(!is_screenshot(&p));
    }

    #[test]
    fn large_files_sorted_desc() {
        let photos = vec![
            photo("a.jpg", Some("C"), 1, 1, 100),
            photo("b.jpg", Some("C"), 1, 1, 500),
            photo("c.jpg", Some("C"), 1, 1, 300),
        ];
        let large = find_large_files(&photos, 2);
        assert_eq!(large.len(), 2);
        assert_eq!(large[0].file_name, "b.jpg");
        assert_eq!(large[1].file_name, "c.jpg");
    }
}
