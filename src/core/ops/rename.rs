//! 批量重命名模板引擎：支持前缀/日期/时间/标签/地点/序号等占位符，
//! 提供预览（不落盘）、执行（含冲突处理）、以及一次撤销。
//!
//! 预览与执行共用 `build_plans()`，两者算出的目标文件名严格一致；
//! 冲突消解不再依赖序号递增（模板不含 `{序号}` 时也能正确追加 `_1`/`_2`）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core::db;
use crate::core::model::{
    OpKind, Photo, RenameContext, RenamePreview, RenamePreviewItem, RenameResult,
};

use super::{move_file, sanitize};

/// 模板支持的占位符（用于 UI 提示）。
pub const PLACEHOLDERS: &[(&str, &str)] = &[
    ("{前缀}", "自定义前缀文本"),
    ("{日期}", "拍摄日期 YYYYMMDD"),
    ("{时间}", "拍摄时间 HHMMSS"),
    ("{标签}", "第一个标签"),
    ("{地点}", "地点文字"),
    ("{序号}", "从 1 开始的序号"),
    ("{序号:3}", "3 位补零序号"),
    ("{原名}", "原文件名（不含扩展名）"),
    ("{扩展名}", "原扩展名（不含点）"),
];

/// 替换值中和：清洗非法字符，并把花括号替换掉，
/// 避免用户前缀 / 标签 / 地点里含 `{日期}` 之类字面量被后续替换二次解析。
fn neutral(s: &str) -> String {
    sanitize(s).replace(['{', '}'], "_")
}

/// 从一张照片构造重命名渲染上下文。
pub fn context_from_photo(photo: &Photo) -> RenameContext {
    // date_taken 形如 "2024-08-15 09:45:00"；异常 EXIF 值（含多字节字符）
    // 用 `.get()` 安全切片，避免按字节切在 UTF-8 字符中间 panic。
    let (date, time) = match &photo.date_taken {
        Some(s) => {
            let date = s.get(..10).unwrap_or(s.as_str()).replace('-', "");
            let time = s.get(11..19).unwrap_or("").replace(':', "");
            (date, time)
        }
        None => {
            // 无 EXIF 时用文件修改时间兜底（秒 → 本地时间格式），避免日期占位符为空。
            chrono::DateTime::from_timestamp_millis(photo.modified_at)
                .map(|dt| {
                    let s = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                    (
                        s.get(..10).unwrap_or("").replace('-', ""),
                        s.get(11..19).unwrap_or("").replace(':', ""),
                    )
                })
                .unwrap_or_default()
        }
    };
    let tag = photo.tag_list().first().map(|s| s.to_string()).unwrap_or_default();
    let location = photo.location.clone().unwrap_or_default();
    let extension = Path::new(&photo.file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    RenameContext {
        date,
        time,
        tag,
        location,
        extension,
    }
}

/// 由上下文渲染模板，返回不带扩展名的文件名主干。
/// `prefix` 用于替换 `{前缀}` 占位符（由 UI 的前缀输入框提供）。
pub fn render_stem(
    pattern: &str,
    ctx: &RenameContext,
    seq: usize,
    original_stem: &str,
    prefix: &str,
) -> String {
    let mut out = pattern.to_string();

    // 序号：支持 {序号:3} / {seq:3}，宽度最大 9
    for token in ["{序号:", "{seq:"] {
        while let Some(start) = out.find(token) {
            let Some(rel_end) = out[start + token.len()..].find('}') else { break };
            let end = start + token.len() + rel_end;
            let width_str = &out[start + token.len()..end];
            let w: usize = width_str.trim().parse().unwrap_or(3).min(9);
            let replacement = format!("{seq:0w$}");
            out.replace_range(start..=end, &replacement);
        }
    }

    let replacements = [
        ("{前缀}", neutral(prefix)),
        ("{prefix}", neutral(prefix)),
        ("{序号}", seq.to_string()),
        ("{seq}", seq.to_string()),
        ("{日期}", neutral(&ctx.date)),
        ("{date}", neutral(&ctx.date)),
        ("{时间}", neutral(&ctx.time)),
        ("{time}", neutral(&ctx.time)),
        ("{标签}", neutral(&ctx.tag)),
        ("{tag}", neutral(&ctx.tag)),
        ("{地点}", neutral(&ctx.location)),
        ("{location}", neutral(&ctx.location)),
        ("{原名}", neutral(original_stem)),
        ("{stem}", neutral(original_stem)),
        ("{扩展名}", neutral(&ctx.extension)),
        ("{ext}", neutral(&ctx.extension)),
    ];
    for (k, v) in replacements {
        out = out.replace(k, &v);
    }

    let cleaned = sanitize(&out);
    if cleaned.is_empty() {
        format!("Photo_{seq}")
    } else {
        cleaned
    }
}

/// 生成完整文件名（含扩展名）。
fn render_filename(
    pattern: &str,
    photo: &Photo,
    ctx: &RenameContext,
    seq: usize,
    prefix: &str,
) -> String {
    let original_stem = Path::new(&photo.file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| photo.file_name.clone());
    let stem = render_stem(pattern, ctx, seq, &original_stem, prefix);
    if ctx.extension.is_empty() {
        stem
    } else {
        format!("{stem}.{}", ctx.extension)
    }
}

/// 拆分文件名为主干 + 扩展名。
fn split_name(name: &str) -> (String, Option<String>) {
    let p = Path::new(name);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .filter(|e| !e.is_empty());
    (stem, ext)
}

/// 在目录内分配一个不冲突的目标路径：磁盘已存在或批内已占用时追加 `_1`、`_2`…
/// `self_path` 是本次重命名的源路径，等于它时不算冲突（原地不动）。
fn allocate_dest(
    dir: &Path,
    base: &str,
    self_path: &Path,
    occupied: &mut HashSet<PathBuf>,
) -> PathBuf {
    let safe_base = sanitize(base);
    let mut candidate = dir.join(&safe_base);
    let (stem, ext) = split_name(&safe_base);
    let mut i = 1usize;
    while (candidate.exists() && candidate.as_path() != self_path) || occupied.contains(&candidate) {
        let name = match &ext {
            Some(e) => format!("{stem}_{i}.{e}"),
            None => format!("{stem}_{i}"),
        };
        candidate = dir.join(sanitize(&name));
        i += 1;
        if i > 1_000_000 {
            break; // 防御性上限：即使模板无序号也保证循环终止
        }
    }
    occupied.insert(candidate.clone());
    candidate
}

struct Plan {
    photo: Photo,
    dest: PathBuf,
    new_name: String,
    /// 目标与源路径相同（模板渲染出原名），无需移动文件。
    unchanged: bool,
}

/// 预览与执行共用的唯一命名计算入口：保证两者结果严格一致。
fn build_plans(
    conn: &Connection,
    photo_ids: &[i64],
    pattern: &str,
    prefix: &str,
) -> Result<Vec<Plan>> {
    let mut occupied: HashSet<PathBuf> = HashSet::new();
    let mut seen_ids: HashSet<i64> = HashSet::new();
    let mut seq = 1usize;
    let mut plans: Vec<Plan> = Vec::new();

    for &id in photo_ids {
        if !seen_ids.insert(id) {
            continue;
        }
        let Some(photo) = db::get_photo_by_id(conn, id)? else {
            continue;
        };
        let ctx = context_from_photo(&photo);
        let base = render_filename(pattern, &photo, &ctx, seq, prefix);
        seq += 1;
        let parent = photo.path.parent().unwrap_or(Path::new("."));
        let dest = allocate_dest(parent, &base, &photo.path, &mut occupied);
        let new_name = dest
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| base.clone());
        let unchanged = dest == photo.path;
        plans.push(Plan {
            photo,
            dest,
            new_name,
            unchanged,
        });
    }
    Ok(plans)
}

/// 预览重命名结果：仅渲染前 `limit` 条（默认 5），总数返回全量。不修改任何文件。
pub fn preview(
    conn: &Connection,
    photo_ids: &[i64],
    pattern: &str,
    prefix: &str,
    limit: usize,
) -> Result<RenamePreview> {
    let plans = build_plans(conn, photo_ids, pattern, prefix)?;
    let items = plans
        .iter()
        .take(limit)
        .map(|p| RenamePreviewItem {
            old_name: p.photo.file_name.clone(),
            new_name: p.new_name.clone(),
        })
        .collect();
    Ok(RenamePreview {
        items,
        total: plans.len(),
    })
}

/// 执行批量重命名：逐张移动文件、更新索引、记录一条可撤销的 Rename 操作日志。
///
/// 冲突处理：批内重名或目标文件已存在时，追加 `_1`、`_2`… 后缀（与预览一致）。
/// 撤销：调用 `undo` 依据 op_entries 逆序移动回原路径。
pub fn execute(
    conn: &Connection,
    photo_ids: &[i64],
    pattern: &str,
    prefix: &str,
) -> Result<RenameResult> {
    if photo_ids.is_empty() {
        return Ok(RenameResult::default());
    }
    let plans = build_plans(conn, photo_ids, pattern, prefix)?;
    if plans.is_empty() {
        return Ok(RenameResult::default());
    }

    // 阶段一：移动文件（目标名已在 plan 阶段统一算好）
    let mut moved: Vec<&Plan> = Vec::new();
    let move_result = (|| -> Result<()> {
        for p in &plans {
            if !p.unchanged {
                move_file(&p.photo.path, &p.dest)?;
                moved.push(p);
            }
        }
        Ok(())
    })();

    if let Err(e) = move_result {
        for p in moved.into_iter().rev() {
            let _ = move_file(&p.dest, &p.photo.path);
        }
        return Err(e).context("重命名文件失败（已尝试回滚）");
    }

    // 阶段二：写库（事务）；失败时把已移动的文件全部移回，避免索引指向旧路径。
    let now = db::now_millis();
    let tx = conn.unchecked_transaction().context("开启重命名事务失败")?;
    let write_result = (|| -> Result<i64> {
        let detail = format!("重命名 {} 张 · 模板 {}", plans.len(), pattern);
        let op_id = db::insert_op_log(&tx, &OpKind::Rename, now, plans.len(), &detail)?;
        for p in &plans {
            db::insert_op_entry(&tx, op_id, &p.photo.path, &p.dest)?;
            db::rename_photo_path(&tx, p.photo.id, &p.dest, &p.new_name)?;
        }
        Ok(op_id)
    })();
    // samples 在 commit 成功后由 plans 直接生成，闭包内不持有临时 Vec。
    let db_result = match write_result {
        Ok(op_id) => tx.commit().context("提交重命名事务失败").map(|_| op_id),
        Err(e) => Err(e),
    };

    let op_id = match db_result {
        Ok(id) => id,
        Err(e) => {
            for p in plans.iter().rev().filter(|p| !p.unchanged) {
                let _ = move_file(&p.dest, &p.photo.path);
            }
            return Err(e).context("写索引失败（已尝试把文件移回原名）");
        }
    };

    let samples = plans
        .iter()
        .take(5)
        .map(|p| RenamePreviewItem {
            old_name: p.photo.file_name.clone(),
            new_name: p.new_name.clone(),
        })
        .collect();

    Ok(RenameResult {
        count: plans.len(),
        op_id,
        samples,
    })
}

/// 撤销一次重命名：依据 op_entries 把文件从 new_path 移回 old_path，并还原索引。
pub fn undo(conn: &Connection, op_id: i64) -> Result<usize> {
    let Some(op) = db::get_op(conn, op_id)? else {
        return Ok(0);
    };
    if op.kind != OpKind::Rename {
        return Ok(0);
    }
    let entries = db::list_op_entries(conn, op_id)?;
    if entries.is_empty() {
        return Ok(0);
    }

    // 阶段一：逆序移动文件回原路径（原地未动的条目跳过）。
    let mut moved: Vec<(&crate::core::model::OpEntry, PathBuf)> = Vec::new();
    let mut occupied: HashSet<PathBuf> = HashSet::new();
    let move_result = (|| -> Result<()> {
        for entry in entries.iter().rev() {
            if entry.new_path == entry.old_path || !entry.new_path.exists() {
                continue;
            }
            let dest = allocate_dest(
                entry.old_path.parent().unwrap_or(Path::new(".")),
                &entry
                    .old_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
                &entry.old_path,
                &mut occupied,
            );
            move_file(&entry.new_path, &dest)?;
            moved.push((entry, dest));
        }
        Ok(())
    })();

    if let Err(e) = move_result {
        for (entry, dest) in moved.iter().rev() {
            let _ = move_file(dest, &entry.new_path);
        }
        return Err(e).context("撤销重命名失败（已尝试回滚）");
    }

    // 阶段二：还原索引；失败时把文件移回 new_path，避免出现无索引的文件。
    let tx = conn.unchecked_transaction().context("开启撤销事务失败")?;
    let write_result = (|| -> Result<()> {
        for (entry, dest) in &moved {
            let name = dest
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(photo) = db::get_photo_by_path(&tx, &entry.new_path)? {
                db::rename_photo_path(&tx, photo.id, dest, &name)?;
            }
        }
        db::delete_op(&tx, op_id)?;
        Ok(())
    })();
    let db_result = match write_result {
        Ok(()) => tx.commit().context("提交撤销事务失败"),
        Err(e) => Err(e),
    };
    if let Err(e) = db_result {
        for (entry, dest) in moved.iter().rev() {
            let _ = move_file(dest, &entry.new_path);
        }
        return Err(e).context("写索引失败（已尝试把文件移回）");
    }

    Ok(moved.len())
}
