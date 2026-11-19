//! 缩略图生成与缓存：把原图缩放到固定最长边，缓存到应用数据目录。
//!
//! key 用内容哈希，重复扫描不重复生成。本期用 `image` crate；M2 可替换为 imageflow 流水线。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::config::Paths;

/// 缩略图最长边像素。
pub const THUMBNAIL_SIZE: u32 = 400;

/// 缓存文件是否有效（存在且非 0 字节）；0 字节多为上次写入中途崩溃的残片。
fn is_valid_cache(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// 确保缩略图存在，返回缓存路径。已存在且完整则直接返回（不重新生成）。
pub fn ensure_thumbnail(paths: &Paths, source: &Path, content_hash: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(&paths.cache_dir)
        .with_context(|| format!("创建缩略图缓存目录失败: {}", paths.cache_dir.display()))?;

    // 哈希失败时用「路径 + 大小 + mtime」退化 key，并加 `fb_` 前缀，
    // 与正常内容哈希命名空间隔离，避免不同来源的文件互相覆盖缓存。
    let name = if content_hash.is_empty() {
        let meta = std::fs::metadata(source)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let key = format!("{:?}|{}|{}", source, meta.len(), mtime);
        format!("fb_{:016x}.jpg", stable_hash(&key))
    } else {
        format!("{}.jpg", content_hash)
    };

    let out = paths.cache_dir.join(name);
    if is_valid_cache(&out) {
        return Ok(out);
    }

    let img = image::open(source)
        .with_context(|| format!("打开原图生成缩略图失败: {}", source.display()))?;
    let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);

    // 原子写：先写同目录临时文件再 rename，避免进程崩溃留下半张损坏的缓存。
    let tmp = paths.cache_dir.join(format!(
        ".tmp-{}-{:016x}.jpg",
        std::process::id(),
        stable_hash(&source.to_string_lossy())
    ));
    thumb
        .save(&tmp)
        .with_context(|| format!("写入缩略图失败: {}", tmp.display()))?;
    let _ = std::fs::remove_file(&out); // 清掉可能存在的 0 字节残片（Windows rename 不覆盖）
    std::fs::rename(&tmp, &out)
        .with_context(|| format!("提交缩略图缓存失败: {}", out.display()))?;
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
