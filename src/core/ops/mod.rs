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

/// 跨平台移动文件：先 `rename`（同盘瞬时），跨盘失败时退化 copy+remove。
/// 安全约束：绝不覆盖已存在的目标；copy 成功但删除源失败时清掉已写副本，
/// 避免留下无人管理的孤儿副本。
pub(crate) fn move_file(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建目标目录失败: {}", parent.display()))?;
    }
    // Windows 的 rename 不覆盖已存在目标，Unix 会覆盖；统一在此显式拒绝。
    if dst.exists() {
        anyhow::bail!("目标文件已存在，拒绝覆盖: {}", dst.display());
    }
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(src, dst)
                .with_context(|| format!("复制文件失败: {} → {}", src.display(), dst.display()))?;
            if let Err(e) = std::fs::remove_file(src) {
                // 源与目标各存一份时，宁可保留源文件、删掉刚写的副本。
                let _ = std::fs::remove_file(dst);
                return Err(e).with_context(|| {
                    format!("删除源文件失败（已清理副本）: {}", src.display())
                });
            }
            Ok(())
        }
    }
}

/// 在目录内生成不冲突的目标路径：若已存在则追加 `_1`、`_2`…（保留扩展名）。
/// 只接受文件名片段，先清洗路径分隔符，防止路径穿越或绝对路径逃逸。
pub(crate) fn unique_dest(dir: &Path, file_name: &str) -> PathBuf {
    let safe = sanitize(file_name);
    let candidate = dir.join(&safe);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(&safe);
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

/// 清洗文件名片段：去除 Windows/Unix 非法字符、控制字符、保留设备名与尾随点/空格。
pub(crate) fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0' => '_',
            c if (c as u32) < 0x20 || c.is_whitespace() => '_',
            c => c,
        })
        .collect::<String>();
    let cleaned: String = cleaned
        .trim_matches('_')
        .trim_end_matches(|c| c == '.' || c == ' ')
        .to_string();
    if cleaned.is_empty() {
        return "_".to_string();
    }
    // Windows 保留设备名（含扩展名形式，如 CON.txt 同样非法）。
    let stem = cleaned
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    );
    if reserved {
        format!("_{cleaned}")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "akashic_ops_unit_{}_{}_{}",
            name,
            std::process::id(),
            SEQ.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 回归：rename 阶段若目标已存在必须拒绝，绝不覆盖用户既有文件。
    #[test]
    fn move_file_never_overwrites_existing_target() {
        let d = tmp("move");
        let src = d.join("a.jpg");
        let dst = d.join("b.jpg");
        std::fs::write(&src, b"new").unwrap();
        std::fs::write(&dst, b"original").unwrap();

        assert!(move_file(&src, &dst).is_err(), "目标已存在必须拒绝");
        assert_eq!(std::fs::read(&dst).unwrap(), b"original", "既有文件不得被覆盖");
        assert!(src.exists(), "失败时源文件应保留");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 回归：文件名清洗要处理 Windows 非法字符、控制字符、保留设备名与尾随点/空格。
    #[test]
    fn sanitize_blocks_illegal_names() {
        let s = sanitize("  a/b:c*?\"<>|\\x  ");
        for c in ['/', '\\', ':', '*', '?', '"', '<', '>', '|'] {
            assert!(!s.contains(c), "非法字符 {c} 应被清洗: {s}");
        }
        assert_eq!(sanitize("CON"), "_CON");
        assert_eq!(sanitize("NUL.txt"), "_NUL.txt");
        assert_eq!(sanitize(""), "_");
        assert_eq!(sanitize(".."), "_");
        assert_eq!(sanitize("a\u{0001}b"), "a_b");
        assert_eq!(sanitize("name. "), "name");
    }

    /// 回归：unique_dest 只接受文件名片段，不得让 `..` 逃出目标目录。
    #[test]
    fn unique_dest_cannot_escape_target_dir() {
        let d = tmp("traversal");
        let p = unique_dest(&d, "../escape.jpg");
        assert_eq!(p.parent().unwrap(), d.as_path(), "不得逃出目标目录");
        assert_eq!(p.file_name().unwrap().to_string_lossy(), ".._escape.jpg");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 回归：unique_dest 在目标已存在时追加 _1 后缀（不依赖模板里的序号）。
    #[test]
    fn unique_dest_appends_suffix_on_conflict() {
        let d = tmp("unique");
        std::fs::write(d.join("a.jpg"), b"x").unwrap();
        let p = unique_dest(&d, "a.jpg");
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "a_1.jpg");
        let _ = std::fs::remove_dir_all(&d);
    }
}
