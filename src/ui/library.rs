//! UI 数据适配层：把 core 索引数据转换成相册库 UI 可直接消费的形状。
//!
//! 依赖 core，不直接访问文件系统（缩略图路径由 core 提供）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::db;
use crate::core::model::{Folder, Photo};
use rusqlite::Connection;

/// 相册库当前展示的数据快照。
#[derive(Clone, Default)]
pub struct LibraryData {
    /// 全部照片
    pub photos: Vec<Photo>,
    /// 从索引路径派生的目录树
    pub folders: Vec<Folder>,
    /// 当前选中的目录路径（None 表示全部）
    pub active_folder: Option<PathBuf>,
    /// 搜索关键词
    pub keyword: String,
    /// 是否已从真实索引加载（false 时 UI 显示示例数据）
    pub loaded: bool,
    /// 扫描根目录数量
    pub root_count: usize,
    /// 摘要：总张数与占用空间
    pub total_count: i64,
    pub total_bytes: i64,
}

impl LibraryData {
    /// 从数据库重新加载全部照片与目录树。
    pub fn reload(conn: &Connection) -> anyhow::Result<Self> {
        let photos = db::list_photos(conn)?;
        let roots = db::list_scan_roots(conn)?;
        let folders = build_folder_tree(conn, &roots)?;
        let total_count = db::count_photos(conn)?;
        let total_bytes = db::total_size_bytes(conn)?;
        Ok(Self {
            photos,
            folders,
            active_folder: None,
            keyword: String::new(),
            loaded: true,
            root_count: roots.len(),
            total_count,
            total_bytes,
        })
    }

    /// 按当前选中目录 + 搜索关键词过滤照片。
    pub fn visible_photos(&self) -> Vec<&Photo> {
        self.photos
            .iter()
            .filter(|p| match &self.active_folder {
                None => true,
                Some(root) => p.path.starts_with(root),
            })
            .filter(|p| {
                if self.keyword.is_empty() {
                    return true;
                }
                let k = self.keyword.to_lowercase();
                p.file_name.to_lowercase().contains(&k)
                    || p.location.as_deref().unwrap_or("").to_lowercase().contains(&k)
                    || p.tags.to_lowercase().contains(&k)
            })
            .collect()
    }

    /// 缩略图显示路径：优先缓存文件，无则 None（UI 用色块占位）。
    pub fn thumbnail_for(&self, photo: &Photo) -> Option<PathBuf> {
        photo.thumbnail_path.clone().filter(|p| p.exists())
    }
}

/// 从索引照片路径派生两层目录树（根目录 → 一级子目录）。
fn build_folder_tree(conn: &Connection, roots: &[crate::core::model::ScanRoot]) -> anyhow::Result<Vec<Folder>> {
    let photos = db::list_photos(conn)?;
    let mut tree: Vec<Folder> = Vec::new();

    for root in roots {
        let root_path = &root.path;
        let root_name = root_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root_path.to_string_lossy().to_string());

        let mut children: BTreeMap<String, usize> = BTreeMap::new();
        let mut root_count = 0usize;
        for photo in &photos {
            if !photo.path.starts_with(root_path) {
                continue;
            }
            root_count += 1;
            if let Ok(rel) = photo.path.strip_prefix(root_path) {
                let sub = rel
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "（根目录）".to_string());
                *children.entry(sub).or_insert(0) += 1;
            }
        }

        let child_nodes: Vec<Folder> = children
            .into_iter()
            .map(|(name, count)| Folder {
                id: format!("{}|{}", root.id, name),
                name: name.clone(),
                path: root_path.join(&name),
                photo_count: count,
                children: vec![],
            })
            .collect();

        tree.push(Folder {
            id: format!("root:{}", root.id),
            name: root_name,
            path: root_path.clone(),
            photo_count: root_count,
            children: child_nodes,
        });
    }
    Ok(tree)
}

/// 目录节点在深度优先展开顺序中的扁平索引（供 UI 展开状态使用）。
pub fn folder_flat_ids(folders: &[Folder]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[Folder], out: &mut Vec<String>) {
        for n in nodes {
            out.push(n.id.clone());
            walk(&n.children, out);
        }
    }
    walk(folders, &mut out);
    out
}

/// 根据扁平索引找到目录节点（不可展开的叶子返回 None 场景由调用方处理）。
pub fn find_folder<'a>(folders: &'a [Folder], id: &str) -> Option<&'a Folder> {
    fn walk<'a>(nodes: &'a [Folder], id: &str) -> Option<&'a Folder> {
        for n in nodes {
            if n.id == id {
                return Some(n);
            }
            if let Some(f) = walk(&n.children, id) {
                return Some(f);
            }
        }
        None
    }
    walk(folders, id)
}

/// 路径是否位于指定目录之下（UI 过滤用）。
pub fn is_under(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}
