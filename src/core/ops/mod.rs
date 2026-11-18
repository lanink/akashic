//! 批量操作层：回收站、重命名、智能归档，不依赖 gpui，可独立单元测试。
//!
//! 安全约束（来自 PRD）：普通删除一律"移动"到回收站，绝不 `remove_file` 用户原文件；
//! 只有「清空回收站 / 清理过期项」这类用户显式动作才会真正删除，且只删回收站内的副本。
//! 归档的复制模式撤销时也只删除本次生成的归档副本，绝不删除用户源文件。

pub mod archive;
pub mod rename;
pub mod trash;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// 跨平台移动文件：先 `rename`（同盘瞬时），跨盘失败时退化copy+remove。
/// 注意：copy+remove 只在目标已完整写入后才删除源文件，不会造成数据丢失。
pub(crate) fn move_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建目标目录失败: {}", parent.display()))?;
    }
    match std::fs::rename(src, dst) {
        Ok(()) => return Ok(()),
        Err(_) => {
            std::fs::copy(src, dst)
                .with_context(|| format!("复制文件失败: {} → {}", src.display(), dst.display()))?;
            std::fs::remove_file(src)
                .with_context(|| format!("删除源文件失败: {}", src.display()))?;
            Ok(())
        }
    }
}

/// 在目录内生成不冲突的目标路径：若已存在则追加 `_1`、`_2`…（保留扩展名）。
pub(crate) fn unique_dest(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string());
    let mut i = 1usize;
    loop {
        let name = match &ext {
            Some(e) if !e.is_empty() => format!("{stem}_{i}.{e}"),
            _ => format!("{stem}_{i}"),
        };
        let p = dir.join(&name);
        if !p.exists() {
            return p;
        }
        i += 1;
    }
}

/// 清洗文件名片段：去除 Windows/Unix 非法字符，替换空白为下划线。
pub(crate) fn sanitize(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0' => '_',
            c if c.is_whitespace() => '_',
            c => c,
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
