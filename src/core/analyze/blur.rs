//! 模糊检测：灰度缩放后用 Laplacian 方差评分。
//!
//! 方差越低表示图像高频细节越少（手抖 / 失焦），越低越模糊。
//! 不依赖 gpui，可独立单元测试。

use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::GrayImage;

/// 分析时统一缩放到的边长（牺牲绝对分辨率换取跨图片的阈值稳定）。
pub const ANALYSIS_SIZE: u32 = 256;

/// Laplacian 方差低于此值判定为模糊。
/// 阈值针对 ANALYSIS_SIZE 的灰度图经验设定，可通过测试样本微调。
pub const BLUR_THRESHOLD: f64 = 100.0;

/// 计算一张图片的模糊评分（Laplacian 方差，越大越清晰）。
pub fn blur_score(path: &Path) -> Result<f64> {
    let img = image::open(path)
        .with_context(|| format!("打开图片计算模糊分失败: {}", path.display()))?;
    let gray = img.to_luma8();
    Ok(laplacian_variance(&gray))
}

/// 是否判定为模糊。
pub fn is_blurry(score: f64) -> bool {
    score < BLUR_THRESHOLD
}

/// 对灰度图做统一缩放后计算 Laplacian 方差。
///
/// 分离为独立函数便于合成样本测试（无需落盘）。
pub fn laplacian_variance(gray: &GrayImage) -> f64 {
    let resized: GrayImage =
        image::imageops::resize(gray, ANALYSIS_SIZE, ANALYSIS_SIZE, FilterType::Triangle);
    let (w, h) = resized.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }

    // 3x3 Laplacian 卷积：4*c - l - r - u - d
    let mut values: Vec<f64> = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = resized.get_pixel(x, y).0[0] as f64;
            let l = resized.get_pixel(x - 1, y).0[0] as f64;
            let r = resized.get_pixel(x + 1, y).0[0] as f64;
            let u = resized.get_pixel(x, y - 1).0[0] as f64;
            let d = resized.get_pixel(x, y + 1).0[0] as f64;
            values.push(4.0 * c - l - r - u - d);
        }
    }

    if values.is_empty() {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    variance
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};

    /// 生成一张纯灰度测试图。
    fn solid_gray(value: u8) -> GrayImage {
        ImageBuffer::from_pixel(ANALYSIS_SIZE, ANALYSIS_SIZE, Luma([value]))
    }

    #[test]
    fn blurred_image_has_low_variance() {
        // 纯色图几乎没有高频细节，方差接近 0
        let score = laplacian_variance(&solid_gray(128));
        assert!(score < BLUR_THRESHOLD, "纯色图方差应低于阈值: {score}");
        assert!(is_blurry(score));
    }

    #[test]
    fn sharp_image_has_high_variance() {
        // 高频棋盘格是典型“清晰”样本，方差应显著高于阈值
        let mut img = solid_gray(0);
        for y in 0..ANALYSIS_SIZE {
            for x in 0..ANALYSIS_SIZE {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                img.put_pixel(x, y, Luma([v]));
            }
        }
        let score = laplacian_variance(&img);
        assert!(score > BLUR_THRESHOLD, "棋盘格方差应高于阈值: {score}");
        assert!(!is_blurry(score));
    }

    #[test]
    fn tiny_image_is_safe() {
        let tiny: GrayImage = ImageBuffer::new(2, 2);
        assert_eq!(laplacian_variance(&tiny), 0.0);
    }
}
