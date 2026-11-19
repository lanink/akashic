//! 回收站：删除（移动到回收站目录）、恢复、清空、30 天过期清理。
//! 全程不永久删除用户原文件，除非用户显式调用 empty_trash / purge_expired。

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core::config::Paths;
use crate::core::db;
use crate::core::model::{OpKind, TrashItem, TrashResult};

use super::{move_file, unique_dest};

/// 回收站文件名的前缀，用于还原时识别。
const TRASH_PREFIX: &str = "trash_";

/// 回收站默认保留天数（未设置或设置非法时回退）。
pub const DEFAULT_RETENTION_DAYS: i64 = 30;
/// 允许设置的保留天数下限。
pub const RETENTION_MIN_DAYS: i64 = 1;
/// 允许设置的保留天数上限。
pub const RETENTION_MAX_DAYS: i64 = 3650;
/// 保留天数在 settings 表中的键。
pub const RETENTION_SETTING_KEY: &str = "trash.retention_days";

/// 读取用户设置的回收站保留天数；未设置、无法解析或越界时回退到默认 30 天。
pub fn retention_days(conn: &Connection) -> i64 {
    match db::get_setting(conn, RETENTION_SETTING_KEY) {
        Ok(Some(v)) => v
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|d| (RETENTION_MIN_DAYS..=RETENTION_MAX_DAYS).contains(d))
            .unwrap_or(DEFAULT_RETENTION_DAYS),
        _ => DEFAULT_RETENTION_DAYS,
    }
}

/// 写入回收站保留天数；超出 1..=3650 时返回错误且不写库。
pub fn set_retention_days(conn: &Connection, days: i64) -> Result<()> {
    if !(RETENTION_MIN_DAYS..=RETENTION_MAX_DAYS).contains(&days) {
        anyhow::bail!(
            "保留天数需在 {}..{} 之间",
            RETENTION_MIN_DAYS,
            RETENTION_MAX_DAYS
        );
    }
    db::set_setting(conn, RETENTION_SETTING_KEY, &days.to_string())
}

/// 把若干照片移动到回收站（不永久删除）。
///
/// 流程：逐张把文件 rename 到 `paths.trash_dir`（冲突自动加后缀）→ 记录一条
/// 含多条 op_entries 的 Trash 操作日志 → 把元数据快照写入 trash_items →
/// 从 photos 表移除索引记录。DB 写操作在一个事务内完成。
pub fn trash_photos(
    paths: &Paths,
    conn: &Connection,
    photo_ids: &[i64],
) -> Result<TrashResult> {
    if photo_ids.is_empty() {
        return Ok(TrashResult::default());
    }
    std::fs::create_dir_all(&paths.trash_dir)
        .with_context(|| format!("创建回收站目录失败: {}", paths.trash_dir.display()))?;

    let now = db::now_millis();
    let mut moved: Vec<(TrashItem, String)> = Vec::new(); // (item, detail)
    let mut bytes = 0i64;

    // 阶段一：移动文件（文件操作不可回滚，先做；失败时已移动的文件尝试移回）
    let result = (|| -> Result<()> {
        for &id in photo_ids {
            let photo = match db::get_photo_by_id(conn, id)? {
                Some(p) => p,
                None => continue,
            };
            let base_name = format!(
                "{}{}_{}",
                TRASH_PREFIX,
                now,
                photo.file_name
            );
            let dest = unique_dest(&paths.trash_dir, &base_name);
            move_file(&photo.path, &dest)?;

            bytes += photo.size_bytes;
            let item = TrashItem {
                id: 0,
                op_id: 0,
                original_path: photo.path.clone(),
                trash_path: dest,
                trashed_at: now,
                photo,
            };
            moved.push((item, String::new()));
        }
        Ok(())
    })();

    if let Err(e) = result {
        // 尽量把已移动的文件移回原位，避免"删了找不到"
        for (item, _) in &moved {
            let _ = move_file(&item.trash_path, &item.original_path);
        }
        return Err(e).context("移动文件到回收站失败（已尝试还原）");
    }

    if moved.is_empty() {
        return Ok(TrashResult::default());
    }

    // 阶段二：写库（事务）
    let tx = conn.unchecked_transaction().context("开启删除事务失败")?;
    let detail = format!("删除 {} 张到回收站", moved.len());
    let op_id = db::insert_op_log(&tx, &OpKind::Trash, now, moved.len(), &detail)?;
    for (item, _) in &mut moved {
        item.op_id = op_id;
        db::insert_op_entry(&tx, op_id, &item.original_path, &item.trash_path)?;
        db::insert_trash_item(&tx, item)?;
        db::delete_photo_by_path(&tx, &item.original_path)?;
    }
    tx.commit().context("提交删除事务失败")?;

    Ok(TrashResult {
        count: moved.len(),
        bytes,
        op_id,
    })
}

/// 从回收站恢复若干项目到原路径。
/// 若原路径已被占用，则在原目录生成带后缀的新文件名，避免覆盖现有文件。
pub fn restore_items(
    paths: &Paths,
    conn: &Connection,
    trash_ids: &[i64],
) -> Result<usize> {
    if trash_ids.is_empty() {
        return Ok(0);
    }
    let now = db::now_millis();
    let mut restored: Vec<(TrashItem, std::path::PathBuf)> = Vec::new(); // (item, final_path)

    let tx = conn.unchecked_transaction().context("开启恢复事务失败")?;
    let move_result = (|| -> Result<()> {
        for &id in trash_ids {
            let item = match db::get_trash_item(&tx, id)? {
                Some(it) => it,
                None => continue,
            };
            let original_dir = item.original_path.parent().unwrap_or(Path::new("/"));
            let file_name = item.photo.file_name.clone();
            let dest = unique_dest(original_dir, &file_name);
            move_file(&item.trash_path, &dest)?;
            restored.push((item, dest));
        }
        Ok(())
    })();

    if let Err(e) = move_result {
        for (item, dest) in &restored {
            let _ = move_file(dest, &item.trash_path);
        }
        return Err(e).context("恢复文件失败（已尝试还原到回收站）");
    }

    let count = restored.len();
    let detail = format!("从回收站恢复 {count} 张");
    let op_id = db::insert_op_log(&tx, &OpKind::Restore, now, count, &detail)?;
    for (item, dest) in &restored {
        let mut photo = item.photo.clone();
        photo.path = dest.clone();
        photo.file_name = dest
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| photo.file_name.clone());
        db::restore_photo(&tx, &photo, None)?;
        db::insert_op_entry(&tx, op_id, &item.trash_path, dest)?;
        db::delete_trash_item(&tx, item.id)?;
    }
    tx.commit().context("提交恢复事务失败")?;

    let _ = paths; // paths 仅用于接口一致性（目录已由 trash_photos 创建）
    Ok(count)
}

/// 清空回收站：永久删除回收站目录内的全部文件与记录。
/// 这是用户显式动作，是唯一允许永久删除的路径之一。
pub fn empty_trash(paths: &Paths, conn: &Connection) -> Result<usize> {
    let items = db::list_trash(conn)?;
    let count = items.len();
    let now = db::now_millis();

    let tx = conn.unchecked_transaction().context("开启清空事务失败")?;
    for item in &items {
        if item.trash_path.exists() {
            std::fs::remove_file(&item.trash_path).with_context(|| {
                format!("删除回收站文件失败: {}", item.trash_path.display())
            })?;
        }
        db::delete_trash_item(&tx, item.id)?;
    }
    let detail = format!("清空回收站 {count} 张");
    db::insert_op_log(&tx, &OpKind::EmptyTrash, now, count, &detail)?;
    tx.commit().context("提交清空事务失败")?;

    let _ = paths;
    Ok(count)
}

/// 清理超过保留期的回收站项目（默认 30 天）。
pub fn purge_expired(
    conn: &Connection,
    retention_days: i64,
) -> Result<usize> {
    let now_ms = db::now_millis();
    let cutoff = now_ms - retention_days * 24 * 60 * 60 * 1000;
    let expired = db::list_expired_trash(conn, cutoff)?;
    let count = expired.len();

    let tx = conn.unchecked_transaction().context("开启清理事务失败")?;
    for item in &expired {
        if item.trash_path.exists() {
            std::fs::remove_file(&item.trash_path).with_context(|| {
                format!("清理过期文件失败: {}", item.trash_path.display())
            })?;
        }
        db::delete_trash_item(&tx, item.id)?;
    }
    let detail = format!("清理 {} 天前项目 {count} 张", retention_days);
    db::insert_op_log(&tx, &OpKind::Purge, now_ms, count, &detail)?;
    tx.commit().context("提交清理事务失败")?;

    Ok(count)
}

/// 撤销一次删除操作：把该 op 的所有条目从回收站还原。
pub fn undo_trash(paths: &Paths, conn: &Connection, op_id: i64) -> Result<usize> {
    let op = db::get_op(conn, op_id)?;
    match op {
        Some(o) if o.kind == OpKind::Trash => {
            let ids = db::list_trash_by_op(conn, op_id)?;
            let n = restore_items(paths, conn, &ids)?;
            db::delete_op(conn, op_id)?;
            Ok(n)
        }
        _ => Ok(0),
    }
}
