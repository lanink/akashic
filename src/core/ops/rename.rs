//! 批量重命名模板引擎：支持前缀/日期/时间/标签/地点/序号等占位符，
//! 提供预览（不落盘）、执行（含冲突处理）、以及一次撤销。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core::db;
use crate::core::model::{
    OpKind, Photo, RenameContext, RenamePreview, RenamePreviewItem, RenameResult,
};

use super::{move_file, sanitize, unique_dest};

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

/// 从一张照片构造重命名渲染上下文。
pub fn context_from_photo(photo: &Photo) -> RenameContext {
    // date_taken 形如 "2024-08-15 09:45:00"；缺失时用修改时间粗化（这里留空，由渲染兜底）
    let (date, time) = match &photo.date_taken {
        Some(s) if s.len() >= 19 => (
            s[0..10].replace('-', ""),
            s[11..19].replace(':', ""),
        ),
        Some(s) if s.len() >= 10 => (s[0..10].replace('-', ""), String::new()),
        _ => (String::new(), String::new()),
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
    for (token, width) in [
        ("{序号:", 0usize),
        ("{seq:", 0usize),
    ] {
        while let Some(start) = out.find(token) {
            let Some(rel_end) = out[start + token.len()..].find('}') else { break };
            let end = start + token.len() + rel_end;
            let width_str = &out[start + token.len()..end];
            let w: usize = width_str.trim().parse().unwrap_or(3).min(9);
            let replacement = format!("{seq:0w$}");
            out.replace_range(start..=end, &replacement);
            let _ = width;
        }
    }

    let replacements = [
        ("{前缀}", sanitize(prefix)),
        ("{prefix}", sanitize(prefix)),
        ("{序号}", seq.to_string()),
        ("{seq}", seq.to_string()),
        ("{日期}", ctx.date.clone()),
        ("{date}", ctx.date.clone()),
        ("{时间}", ctx.time.clone()),
        ("{time}", ctx.time.clone()),
        ("{标签}", sanitize(&ctx.tag)),
        ("{tag}", sanitize(&ctx.tag)),
        ("{地点}", sanitize(&ctx.location)),
        ("{location}", sanitize(&ctx.location)),
        ("{原名}", sanitize(original_stem)),
        ("{stem}", sanitize(original_stem)),
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

/// 预览重命名结果：仅渲染前 `limit` 条（默认 5），总数返回全量。不修改任何文件。
pub fn preview(
    conn: &Connection,
    photo_ids: &[i64],
    pattern: &str,
    prefix: &str,
    limit: usize,
) -> Result<RenamePreview> {
    let mut items = Vec::new();
    let mut used = std::collections::HashSet::new();
    let mut seq = 1usize;

    for &id in photo_ids {
        let photo = match db::get_photo_by_id(conn, id)? {
            Some(p) => p,
            None => continue,
        };
        let ctx = context_from_photo(&photo);
        let mut name = render_filename(pattern, &photo, &ctx, seq, prefix);
        // 预览阶段也做批内冲突消解，保证展示与执行一致
        while used.contains(&name) {
            seq += 1;
            name = render_filename(pattern, &photo, &ctx, seq, prefix);
        }
        used.insert(name.clone());
        if items.len() < limit {
            items.push(RenamePreviewItem {
                old_name: photo.file_name.clone(),
                new_name: name,
            });
        }
        seq += 1;
    }

    Ok(RenamePreview {
        items,
        total: used.len(),
    })
}

/// 执行批量重命名：逐张移动文件、更新索引、记录一条可撤销的 Rename 操作日志。
///
/// 冲突处理：批内重名或目标文件已存在时，追加 `_1`、`_2`… 后缀。
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

    // 阶段一：计算全部目标名并移动文件（先算后移，避免边移边改导致读取失败）
    struct Plan {
        photo: Photo,
        dest: PathBuf,
        new_name: String,
    }
    let mut plans: Vec<Plan> = Vec::new();
    let mut used_in_batch: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seq = 1usize;

    let move_result = (|| -> Result<()> {
        for &id in photo_ids {
            let photo = match db::get_photo_by_id(conn, id)? {
                Some(p) => p,
                None => continue,
            };
            let ctx = context_from_photo(&photo);
            let mut new_name = render_filename(pattern, &photo, &ctx, seq, prefix);
            while used_in_batch.contains(&new_name) {
                seq += 1;
                new_name = render_filename(pattern, &photo, &ctx, seq, prefix);
            }
            used_in_batch.insert(new_name.clone());

            let parent = photo.path.parent().unwrap_or(Path::new("."));
            let dest = unique_dest(parent, &new_name);
            let final_name = dest
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| new_name.clone());
            move_file(&photo.path, &dest)?;

            plans.push(Plan {
                photo,
                dest,
                new_name: final_name,
            });
            seq += 1;
        }
        Ok(())
    })();

    if let Err(e) = move_result {
        // 回滚：已移动的改回原路径
        for p in &plans {
            let _ = move_file(&p.dest, &p.photo.path);
        }
        return Err(e).context("重命名文件失败（已尝试回滚）");
    }

    if plans.is_empty() {
        return Ok(RenameResult::default());
    }

    // 阶段二：写库（事务）
    let now = db::now_millis();
    let tx = conn.unchecked_transaction().context("开启重命名事务失败")?;
    let detail = format!("重命名 {} 张 · 模板 {}", plans.len(), pattern);
    let op_id = db::insert_op_log(&tx, &OpKind::Rename, now, plans.len(), &detail)?;
    let mut samples = Vec::new();
    for p in &plans {
        db::insert_op_entry(&tx, op_id, &p.photo.path, &p.dest)?;
        db::rename_photo_path(&tx, p.photo.id, &p.dest, &p.new_name)?;
        if samples.len() < 5 {
            samples.push(RenamePreviewItem {
                old_name: p.photo.file_name.clone(),
                new_name: p.new_name.clone(),
            });
        }
    }
    tx.commit().context("提交重命名事务失败")?;

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

    // 阶段一：移动文件回原路径
    let mut moved: Vec<(&crate::core::model::OpEntry, PathBuf)> = Vec::new();
    let move_result = (|| -> Result<()> {
        for entry in &entries {
            if !entry.new_path.exists() {
                continue;
            }
            let dest = unique_dest(
                entry.old_path.parent().unwrap_or(Path::new(".")),
                &entry
                    .old_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
            );
            move_file(&entry.new_path, &dest)?;
            moved.push((entry, dest));
        }
        Ok(())
    })();

    if let Err(e) = move_result {
        for (entry, dest) in &moved {
            let _ = move_file(dest, &entry.new_path);
        }
        return Err(e).context("撤销重命名失败（已尝试回滚）");
    }

    // 阶段二：还原索引
    let tx = conn.unchecked_transaction().context("开启撤销事务失败")?;
    for (entry, dest) in &moved {
        let name = dest
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Some(mut photo) = db::get_photo_by_path(&tx, &entry.new_path)? {
            photo.path = dest.clone();
            photo.file_name = name;
            db::rename_photo_path(&tx, photo.id, dest, &photo.file_name)?;
        }
    }
    db::delete_op(&tx, op_id)?;
    tx.commit().context("提交撤销事务失败")?;

    Ok(moved.len())
}
