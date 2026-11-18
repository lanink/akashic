//! 智能归档（M4）：按年 / 年月 / 故事 / 标签生成目标目录树，支持移动与复制两种模式。
//! 执行前给出完整目录树预览（只读、不落盘），执行后可通过操作日志一次撤销。
//!
//! 安全约束：
//! - 绝不覆盖已有文件：预览与执行都会对重名追加 `_1`/`_2` 后缀。
//! - 移动模式只搬动文件本身，撤销会把文件移回原路径并还原索引。
//! - 复制模式保留源文件，撤销时只删除本次生成的归档副本（副本不是用户原文件）。
//! - 所有批量写库在一个事务内完成；文件操作失败时尽力回滚已处理的项。

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core::db;
use crate::core::model::{OpKind, Photo};

use super::{move_file, sanitize, unique_dest};

/// 归档目录规则。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArchiveRule {
    /// `<目标>/<年>/<年-月>/<文件>`
    #[default]
    ByYearMonth,
    /// `<目标>/<年>/<文件>`
    ByYear,
    /// `<目标>/<年>/<年-月 地点或标签>/<文件>`
    ByStory,
    /// `<目标>/<标签>/<文件>`
    ByTag,
}

impl ArchiveRule {
    pub const ALL: &'static [ArchiveRule] = &[
        ArchiveRule::ByYearMonth,
        ArchiveRule::ByYear,
        ArchiveRule::ByStory,
        ArchiveRule::ByTag,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ArchiveRule::ByYearMonth => "按年 / 月",
            ArchiveRule::ByYear => "按年",
            ArchiveRule::ByStory => "按故事（时间 + 地点/标签）",
            ArchiveRule::ByTag => "按标签",
        }
    }

    pub fn example(&self) -> &'static str {
        match self {
            ArchiveRule::ByYearMonth => "归档/2024/2024-08/IMG_0001.jpg",
            ArchiveRule::ByYear => "归档/2024/IMG_0001.jpg",
            ArchiveRule::ByStory => "归档/2024/2024-08_富士山/IMG_0001.jpg",
            ArchiveRule::ByTag => "归档/旅行/IMG_0001.jpg",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiveRule::ByYearMonth => "year_month",
            ArchiveRule::ByYear => "year",
            ArchiveRule::ByStory => "story",
            ArchiveRule::ByTag => "tag",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "year" => ArchiveRule::ByYear,
            "story" => ArchiveRule::ByStory,
            "tag" => ArchiveRule::ByTag,
            _ => ArchiveRule::ByYearMonth,
        }
    }
}

/// 归档模式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArchiveMode {
    /// 移动文件到归档目录（源文件不再留在原处，可撤销移回）
    #[default]
    Move,
    /// 复制文件到归档目录（源文件保留，撤销会删除副本）
    Copy,
}

impl ArchiveMode {
    pub fn label(&self) -> &'static str {
        match self {
            ArchiveMode::Move => "移动（整理原目录）",
            ArchiveMode::Copy => "复制（保留原文件）",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiveMode::Move => "move",
            ArchiveMode::Copy => "copy",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "copy" => ArchiveMode::Copy,
            _ => ArchiveMode::Move,
        }
    }
}

/// 归档计划中的一条文件映射。
#[derive(Clone, Debug)]
pub struct ArchiveItem {
    pub photo_id: i64,
    pub from_path: PathBuf,
    pub to_path: PathBuf,
    /// 相对 dest_root 的展示路径，用 `/` 分隔（目录树预览依据）
    pub relative: String,
    pub size_bytes: i64,
    /// 是否因目标重名而追加了后缀
    pub conflict: bool,
}

/// 目录树预览节点。
#[derive(Clone, Debug)]
pub struct ArchiveTreeNode {
    pub name: String,
    /// 相对 dest_root 的路径
    pub rel_path: String,
    pub dir_count: usize,
    pub file_count: usize,
    pub total_count: usize,
    pub total_bytes: i64,
    pub files: Vec<String>,
    pub sample: Vec<String>,
    pub children: Vec<ArchiveTreeNode>,
}

/// 一次归档的完整计划（只读计算，供 UI 预览）。
#[derive(Clone, Debug)]
pub struct ArchivePlan {
    pub rule: ArchiveRule,
    pub mode: ArchiveMode,
    pub dest_root: PathBuf,
    pub items: Vec<ArchiveItem>,
    pub tree: Vec<ArchiveTreeNode>,
    pub total_files: usize,
    pub total_bytes: i64,
    pub conflict_count: usize,
}

/// 归档执行结果。
#[derive(Clone, Debug, Default)]
pub struct ArchiveResult {
    pub count: usize,
    pub op_id: i64,
    pub dest_root: PathBuf,
}

/// 从拍摄时间派生 (年, 年-月)；缺失时用「未知日期」兜底。
fn date_parts(photo: &Photo) -> (String, String) {
    match &photo.date_taken {
        Some(s) if s.len() >= 7 => {
            let year = s.get(..4).unwrap_or("未知日期").to_string();
            let ym = s.get(..7).unwrap_or(&year).to_string();
            (year, ym)
        }
        Some(s) if s.len() >= 4 => {
            let year = s.get(..4).unwrap_or("未知日期").to_string();
            (year.clone(), format!("{}_未知月", year))
        }
        _ => ("未知日期".to_string(), "未知日期".to_string()),
    }
}

/// 故事名：地点 → 第一个标签 → 「未命名故事」。
fn story_label(photo: &Photo) -> String {
    let raw = photo
        .location
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .or_else(|| photo.tag_list().first().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "未命名故事".to_string());
    sanitize(&raw)
}

/// 标签名：第一个标签 → 「未分类」。
fn tag_label(photo: &Photo) -> String {
    let raw = photo
        .tag_list()
        .first()
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| "未分类".to_string());
    sanitize(&raw)
}

/// 按规则生成目标目录分段（相对 dest_root），每段已清洗且不含 `.`/`..`。
fn dest_components(rule: ArchiveRule, photo: &Photo) -> Vec<String> {
    let (year, ym) = date_parts(photo);
    let raw: Vec<String> = match rule {
        ArchiveRule::ByYear => vec![year],
        ArchiveRule::ByYearMonth => vec![year, ym],
        ArchiveRule::ByStory => vec![year, format!("{}_{}", ym, story_label(photo))],
        ArchiveRule::ByTag => vec![tag_label(photo)],
    };
    let cleaned: Vec<String> = raw
        .into_iter()
        .map(|s| sanitize(&s))
        .filter(|s| !s.is_empty() && s != "." && s != "..")
        .collect();
    if cleaned.is_empty() {
        vec!["未分类".to_string()]
    } else {
        cleaned
    }
}

/// 预览阶段的去重：目标已存在或批内已占用时，追加 `_1`/`_2`… 后缀。
fn plan_unique(dir: &Path, file_name: &str, used: &mut HashSet<PathBuf>) -> (PathBuf, bool) {
    let candidate = dir.join(file_name);
    if !candidate.exists() && used.insert(candidate.clone()) {
        return (candidate, false);
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = path.extension().map(|e| e.to_string_lossy().to_string());
    let mut i = 1usize;
    loop {
        let name = match &ext {
            Some(e) if !e.is_empty() => format!("{stem}_{i}.{e}"),
            _ => format!("{stem}_{i}"),
        };
        let p = dir.join(&name);
        if !p.exists() && used.insert(p.clone()) {
            return (p, true);
        }
        i += 1;
    }
}

#[derive(Default)]
struct TreeBuilder {
    children: BTreeMap<String, TreeBuilder>,
    /// (文件名, 大小)
    files: Vec<(String, i64)>,
}

fn to_node(name: &str, parent_rel: &str, b: &TreeBuilder) -> ArchiveTreeNode {
    let rel_path = if parent_rel.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent_rel, name)
    };
    let children: Vec<ArchiveTreeNode> = b
        .children
        .iter()
        .map(|(n, c)| to_node(n, &rel_path, c))
        .collect();
    let file_count = b.files.len();
    let total_count = file_count + children.iter().map(|c| c.total_count).sum::<usize>();
    let total_bytes: i64 = b.files.iter().map(|(_, s)| *s).sum::<i64>()
        + children.iter().map(|c| c.total_bytes).sum::<i64>();
    ArchiveTreeNode {
        name: name.to_string(),
        rel_path,
        dir_count: children.len(),
        file_count,
        total_count,
        total_bytes,
        files: b.files.iter().map(|(n, _)| n.clone()).collect(),
        sample: b.files.iter().take(3).map(|(n, _)| n.clone()).collect(),
        children,
    }
}

/// 由文件映射构建完整目录树（相同目录合并，按名称排序）。
fn build_tree(items: &[ArchiveItem]) -> Vec<ArchiveTreeNode> {
    let mut root = TreeBuilder::default();
    for item in items {
        let parts: Vec<&str> = item.relative.split('/').filter(|s| !s.is_empty()).collect();
        let Some((file_name, dirs)) = parts.split_last() else {
            continue;
        };
        let mut node = &mut root;
        for d in dirs {
            node = node.children.entry((*d).to_string()).or_default();
        }
        node.files.push(((*file_name).to_string(), item.size_bytes));
    }
    root.children
        .iter()
        .map(|(name, b)| to_node(name, "", b))
        .collect()
}

/// 生成归档计划：只读数据库与文件系统，不创建目录、不移动任何文件。
///
/// `photo_ids` 为空时返回空计划；`dest_root` 为归档目标根目录。
pub fn plan(
    conn: &Connection,
    photo_ids: &[i64],
    rule: ArchiveRule,
    mode: ArchiveMode,
    dest_root: &Path,
) -> Result<ArchivePlan> {
    let mut items = Vec::new();
    let mut used: HashSet<PathBuf> = HashSet::new();
    let mut total_bytes = 0i64;
    let mut conflict_count = 0usize;

    for &id in photo_ids {
        let photo = match db::get_photo_by_id(conn, id)? {
            Some(p) => p,
            None => continue,
        };
        let comps = dest_components(rule, &photo);
        let dir = comps
            .iter()
            .fold(dest_root.to_path_buf(), |acc, c| acc.join(c));
        let (to_path, conflict) = plan_unique(&dir, &photo.file_name, &mut used);
        let final_name = to_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| photo.file_name.clone());
        let mut rel_parts = comps.clone();
        rel_parts.push(final_name);
        if conflict {
            conflict_count += 1;
        }
        total_bytes += photo.size_bytes;
        items.push(ArchiveItem {
            photo_id: photo.id,
            from_path: photo.path.clone(),
            to_path,
            relative: rel_parts.join("/"),
            size_bytes: photo.size_bytes,
            conflict,
        });
    }

    Ok(ArchivePlan {
        rule,
        mode,
        dest_root: dest_root.to_path_buf(),
        total_files: items.len(),
        total_bytes,
        conflict_count,
        tree: build_tree(&items),
        items,
    })
}

/// 执行归档：先落盘（移动或复制），再事务写 op_log / op_entries / 索引。
/// `plan` 必须来自 `plan()`，保证「目录树预览」与「实际落盘」一致。
pub fn execute(conn: &Connection, plan: &ArchivePlan) -> Result<ArchiveResult> {
    if plan.items.is_empty() {
        return Ok(ArchiveResult::default());
    }

    let kind = match plan.mode {
        ArchiveMode::Move => OpKind::ArchiveMove,
        ArchiveMode::Copy => OpKind::ArchiveCopy,
    };

    // 阶段一：文件操作（失败时尽力回滚已处理项）
    let mut done: Vec<(&ArchiveItem, PathBuf)> = Vec::new();
    let file_result = (|| -> Result<()> {
        for item in &plan.items {
            // 防御：预览之后目标被外部占用时再加后缀，绝不覆盖现有文件。
            let dst = if item.to_path.exists() {
                let parent = item.to_path.parent().unwrap_or(Path::new("."));
                let name = item
                    .to_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                unique_dest(parent, &name)
            } else {
                item.to_path.clone()
            };
            match plan.mode {
                ArchiveMode::Move => move_file(&item.from_path, &dst)?,
                ArchiveMode::Copy => {
                    if let Some(parent) = dst.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("创建归档目录失败: {}", parent.display())
                        })?;
                    }
                    std::fs::copy(&item.from_path, &dst).with_context(|| {
                        format!("复制归档文件失败: {} → {}", item.from_path.display(), dst.display())
                    })?;
                }
            }
            done.push((item, dst));
        }
        Ok(())
    })();

    if let Err(e) = file_result {
        for (item, dst) in &done {
            match plan.mode {
                ArchiveMode::Move => {
                    let _ = move_file(dst, &item.from_path);
                }
                ArchiveMode::Copy => {
                    let _ = std::fs::remove_file(dst);
                }
            }
        }
        return Err(e).context("归档文件失败（已尝试回滚）");
    }

    // 阶段二：写库（事务）
    let now = db::now_millis();
    let tx = conn.unchecked_transaction().context("开启归档事务失败")?;
    let detail = format!(
        "归档 {} 张 · {} · {} → {}",
        done.len(),
        plan.rule.label(),
        plan.mode.label(),
        plan.dest_root.display()
    );
    let op_id = db::insert_op_log(&tx, &kind, now, done.len(), &detail)?;

    for (item, dst) in &done {
        let name = dst
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        db::insert_op_entry(&tx, op_id, &item.from_path, dst)?;
        match plan.mode {
            ArchiveMode::Move => {
                db::rename_photo_path(&tx, item.photo_id, dst, &name)?;
            }
            ArchiveMode::Copy => {
                // 副本也进入索引，便于在相册库看到归档结果；源文件索引保持不变。
                if let Some(mut photo) = db::get_photo_by_id(&tx, item.photo_id)? {
                    photo.id = 0;
                    photo.path = dst.clone();
                    photo.file_name = name;
                    db::restore_photo(&tx, &photo, None)?;
                }
            }
        }
    }
    tx.commit().context("提交归档事务失败")?;

    Ok(ArchiveResult {
        count: done.len(),
        op_id,
        dest_root: plan.dest_root.clone(),
    })
}

/// 撤销一次归档：
/// - 移动模式：把文件移回原路径并还原索引（原路径被占用时加后缀）。
/// - 复制模式：删除本次生成的归档副本与其索引记录，源文件与源索引保持不变。
pub fn undo(conn: &Connection, op_id: i64) -> Result<usize> {
    let Some(op) = db::get_op(conn, op_id)? else {
        return Ok(0);
    };
    let entries = db::list_op_entries(conn, op_id)?;
    if entries.is_empty() {
        return Ok(0);
    }

    match op.kind {
        OpKind::ArchiveMove => {
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
                return Err(e).context("撤销归档失败（已尝试回滚）");
            }

            let tx = conn.unchecked_transaction().context("开启撤销归档事务失败")?;
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
            tx.commit().context("提交撤销归档事务失败")?;
            Ok(moved.len())
        }
        OpKind::ArchiveCopy => {
            let tx = conn.unchecked_transaction().context("开启撤销归档事务失败")?;
            let mut removed = 0usize;
            for entry in &entries {
                // 只删除归档副本，绝不触碰源文件（new_path 必须不同于 old_path）。
                if entry.new_path == entry.old_path {
                    continue;
                }
                if entry.new_path.exists() {
                    std::fs::remove_file(&entry.new_path).with_context(|| {
                        format!("删除归档副本失败: {}", entry.new_path.display())
                    })?;
                }
                db::delete_photo_by_path(&tx, &entry.new_path)?;
                removed += 1;
            }
            db::delete_op(&tx, op_id)?;
            tx.commit().context("提交撤销归档事务失败")?;
            Ok(removed)
        }
        _ => Ok(0),
    }
}
