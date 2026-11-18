//! 相似归组：pHash 感知哈希预筛 + dssim 结构相似度精排 + 并查集分组。
//!
//! 不依赖 gpui，可独立单元测试。pHash 部分为纯算法（合成灰度图即可测试），
//! dssim 精排单独封装，便于在无缩略图测试样本时跳过。

use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::GrayImage;

/// pHash 输入缩放边长（DCT 分辨率）。
const HASH_SIZE: u32 = 32;
/// 取低频区域边长（8x8 = 64 位哈希）。
const LOW_FREQ: usize = 8;
/// pHash 汉明距离阈值：小于等于该值视为候选相似。
pub const HAMMING_THRESHOLD: u32 = 8;
/// dssim 距离阈值：越小越相似，大于该值剔除候选。
pub const DSSIM_THRESHOLD: f64 = 0.03;

/// 一维 DCT-II（可分离 DCT 的行/列变换）。
fn dct_1d(input: &[f64]) -> Vec<f64> {
    let n = input.len();
    let mut out = vec![0.0; n];
    for k in 0..n {
        let mut sum = 0.0;
        for t in 0..n {
            sum += input[t]
                * ((2 * t + 1) as f64 * k as f64 * std::f64::consts::PI / (2.0 * n as f64)).cos();
        }
        let scale = if k == 0 {
            (1.0 / n as f64).sqrt()
        } else {
            (2.0 / n as f64).sqrt()
        };
        out[k] = scale * sum;
    }
    out
}

/// 从灰度图计算 64 位 pHash。
pub fn phash_from_gray(gray: &GrayImage) -> Option<u64> {
    let size = HASH_SIZE as usize;
    let resized: GrayImage =
        image::imageops::resize(gray, HASH_SIZE, HASH_SIZE, FilterType::Triangle);
    let mut mat = vec![0.0; size * size];
    for (i, p) in resized.pixels().enumerate() {
        mat[i] = p.0[0] as f64;
    }

    // 行 DCT
    let mut rows = vec![0.0; size * size];
    for y in 0..size {
        let d = dct_1d(&mat[y * size..(y + 1) * size]);
        rows[y * size..(y + 1) * size].copy_from_slice(&d);
    }
    // 列 DCT
    let mut dct = vec![0.0; size * size];
    for x in 0..size {
        let col: Vec<f64> = (0..size).map(|y| rows[y * size + x]).collect();
        let d = dct_1d(&col);
        for y in 0..size {
            dct[y * size + x] = d[y];
        }
    }

    // 取左上 8x8 低频系数，以中位数为阈值生成 64 位
    let mut vals = Vec::with_capacity(LOW_FREQ * LOW_FREQ);
    for y in 0..LOW_FREQ {
        for x in 0..LOW_FREQ {
            vals.push(dct[y * size + x]);
        }
    }
    let mut sorted = vals.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let mut hash = 0u64;
    for (i, v) in vals.iter().enumerate() {
        if *v > median {
            hash |= 1u64 << i;
        }
    }
    Some(hash)
}

/// 从图片路径计算 pHash。
pub fn phash_from_path(path: &Path) -> Result<u64> {
    let img = image::open(path)
        .with_context(|| format!("打开图片计算 pHash 失败: {}", path.display()))?;
    phash_from_gray(&img.to_luma8())
        .context("计算 pHash 失败")
}

/// 两个哈希的汉明距离。
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// 基于 pHash 汉明距离做并查集分组。
///
/// 输入为 `(照片 id, phash)` 列表，返回每组至少 2 张的照片 id 集合。
pub fn group_by_phash(items: &[(i64, u64)], max_dist: u32) -> Vec<Vec<i64>> {
    struct UnionFind {
        parent: Vec<usize>,
    }
    impl UnionFind {
        fn new(n: usize) -> Self {
            Self {
                parent: (0..n).collect(),
            }
        }
        fn find(&mut self, x: usize) -> usize {
            if self.parent[x] != x {
                let root = self.find(self.parent[x]);
                self.parent[x] = root;
            }
            self.parent[x]
        }
        fn union(&mut self, a: usize, b: usize) {
            let ra = self.find(a);
            let rb = self.find(b);
            if ra != rb {
                self.parent[rb] = ra;
            }
        }
    }

    let n = items.len();
    if n < 2 {
        return Vec::new();
    }
    let mut uf = UnionFind::new(n);
    for i in 0..n {
        for j in (i + 1)..n {
            if hamming(items[i].1, items[j].1) <= max_dist {
                uf.union(i, j);
            }
        }
    }

    use std::collections::HashMap;
    let mut groups: HashMap<usize, Vec<i64>> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        groups.entry(root).or_default().push(items[i].0);
    }
    let mut result: Vec<Vec<i64>> = groups
        .into_values()
        .filter(|g| g.len() >= 2)
        .collect();
    for g in &mut result {
        g.sort_unstable();
    }
    result.sort();
    result
}

/// 用 dssim 计算两张图片的结构差异距离（0 表示完全一致）。
pub fn dssim_distance(a: &Path, b: &Path) -> Result<f64> {
    let attr = dssim::new();
    let img_a = dssim::load_image(&attr, a)
        .map_err(|e| anyhow::anyhow!("dssim 加载图片失败 {}: {}", a.display(), e))?;
    let img_b = dssim::load_image(&attr, b)
        .map_err(|e| anyhow::anyhow!("dssim 加载图片失败 {}: {}", b.display(), e))?;
    let (val, _map) = attr.compare(&img_a, img_b);
    Ok(val.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Luma};

    fn gray_from_fn<F: Fn(u32, u32) -> u8>(w: u32, h: u32, f: F) -> GrayImage {
        let mut img: GrayImage = ImageBuffer::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Luma([f(x, y)]));
            }
        }
        img
    }

    #[test]
    fn identical_images_have_zero_hamming() {
        let g = gray_from_fn(64, 64, |x, y| ((x * 3 + y * 5) % 256) as u8);
        let h1 = phash_from_gray(&g).unwrap();
        let h2 = phash_from_gray(&g).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(hamming(h1, h2), 0);
    }

    #[test]
    fn similar_images_have_small_hamming() {
        let base = |x: u32, y: u32| ((x * 3 + y * 5) % 256) as u8;
        let g1 = gray_from_fn(64, 64, base);
        // 轻微亮度变化，pHash 应基本不变
        let g2 = gray_from_fn(64, 64, |x, y| base(x, y).saturating_add(8));
        let d = hamming(phash_from_gray(&g1).unwrap(), phash_from_gray(&g2).unwrap());
        assert!(d <= HAMMING_THRESHOLD, "相似图汉明距离应较小: {d}");
    }

    #[test]
    fn different_images_have_large_hamming() {
        let g1 = gray_from_fn(64, 64, |x, y| if (x + y) % 2 == 0 { 255 } else { 0 });
        let g2 = gray_from_fn(64, 64, |x, _y| ((x * 7) % 256) as u8);
        let d = hamming(phash_from_gray(&g1).unwrap(), phash_from_gray(&g2).unwrap());
        assert!(d > HAMMING_THRESHOLD, "不同图汉明距离应较大: {d}");
    }

    #[test]
    fn union_find_groups_by_phash() {
        // 三张接近的哈希 + 一张远离的
        let items = vec![
            (1i64, 0b0000_0000_1111_0000u64),
            (2, 0b0000_0000_1111_0001),
            (3, 0b0000_0000_1111_0011),
            (4, 0b1111_1111_0000_0000),
        ];
        let groups = group_by_phash(&items, 2);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![1, 2, 3]);
    }
}
