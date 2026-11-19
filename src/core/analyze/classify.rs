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
/// 刻意不含 4:3 / 3:4 —— 那是普通相机最常见的比例，把它当截图证据会误判相机照片。
const SCREEN_RATIOS: &[f64] = &[
    16.0 / 9.0,   // 横屏
    9.0 / 16.0,   // 竖屏
    19.5 / 9.0,   // 全面屏
    9.0 / 19.5,
    20.0 / 9.0,
    9.0 / 20.0,
];

/// 精确屏幕分辨率（宽×高）。相机不会输出这些尺寸，是「无 EXIF 截图」最强的证据。
const SCREEN_SIZES: &[(i32, i32)] = &[
    (1080, 1920), (1920, 1080), (1170, 2532), (1284, 2778),
    (1440, 3200), (1080, 2400), (750, 1334), (1242, 2208),
    (2560, 1440), (1125, 2436), (828, 1792), (1242, 2688),
    (1536, 2048), (1440, 2560),
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

    // 宽高比接近常见屏幕比例（不含相机常用的 4:3）
    if photo.width > 0 && photo.height > 0 {
        let ratio = photo.height as f64 / photo.width as f64;
        if SCREEN_RATIOS.iter().any(|r| (ratio - r).abs() < 0.03) {
            score += 15;
        }
    }

    // 精确匹配屏幕分辨率：让「无关键词、无 EXIF」的真实截图也能过阈值，
    // 而不必把「无 EXIF」单独加权到会误判所有网络图的程度。
    if SCREEN_SIZES.iter().any(|(w, h)| {
        (photo.width == *w && photo.height == *h)
            || (photo.width == *h && photo.height == *w)
    }) {
        score += 30;
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

    /// 回归：4:3 是普通相机最常见比例，不能被当作截图证据。
    /// 修复前 4:3/3:4 在 SCREEN_RATIOS 中，会让「弱关键词 + 有 EXIF 设备」的
    /// 相机照凑满 50 分被误判成截图。
    #[test]
    fn four_three_ratio_is_not_screenshot_evidence() {
        let p = photo("screen_capture.png", Some("Canon EOS"), 4000, 3000, 3_000_000);
        assert_eq!(screenshot_likelihood(&p), 35);
        assert!(!is_screenshot(&p));
    }

    /// 回归：真实截图常无关键词，但尺寸精确等于屏幕分辨率，应能过阈值。
    /// 修复前只有「无 EXIF 20 + 屏幕比例 15 = 35」，必然漏判。
    #[test]
    fn exact_screen_size_without_keywords_is_screenshot() {
        let p = photo("IMG_9999.png", None, 1080, 1920, 500_000);
        assert!(screenshot_likelihood(&p) >= SCREENSHOT_THRESHOLD);
        assert!(is_screenshot(&p));
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
