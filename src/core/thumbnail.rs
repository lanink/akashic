//! 缩略图生成与缓存：把原图缩放到固定最长边，缓存到应用数据目录。
//!
//! key 用内容哈希，重复扫描不重复生成。本期用 `image` crate；M2 可替换为 imageflow 流水线。

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::config::Paths;

/// 缩略图最长边像素。
pub const THUMBNAIL_SIZE: u32 = 400;

/// 确保缩略图存在，返回缓存路径。已存在则直接返回（不重新生成）。
pub fn ensure_thumbnail(paths: &Paths, source: &Path, content_hash: &str) -> Result<PathBuf> {
    let name = if content_hash.is_empty() {
        // 哈希失败时退化为 路径+mtime 的简单标识，避免覆盖
        let meta = std::fs::metadata(source)?;
        let key = format!("{:?}{}", source, meta.len());
        format!("{:016x}.jpg", stable_hash(&key))
    } else {
        format!("{}.jpg", content_hash)
    };

    let out = paths.cache_dir.join(name);
    if out.exists() {
        return Ok(out);
    }

    let img = image::open(source)?;
    let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    thumb.save(&out)?;
    Ok(out)
}

/// 字符串稳定哈希（FNV-1a 64），仅用于文件名退化场景。
fn stable_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic() {
        assert_eq!(stable_hash("abc"), stable_hash("abc"));
        assert_ne!(stable_hash("abc"), stable_hash("abd"));
    }
}
