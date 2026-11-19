//! 数据模型：真实照片/扫描根目录/目录树/批量操作等结构。
//! 本模块不依赖 gpui，可独立单元测试；M5 起不再包含任何 M0 模拟数据。

use std::path::PathBuf;

/// 一张真实照片的索引记录（对应 SQLite `photos` 表一行）。
#[derive(Clone, Debug)]
pub struct Photo {
    pub id: i64,
    /// 绝对路径
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: i64,
    /// EXIF 拍摄时间（格式 YYYY-MM-DD HH:MM:SS）；无 EXIF 时退化为文件修改时间
    pub date_taken: Option<String>,
    pub modified_at: i64,
    pub width: i32,
    pub height: i32,
    pub device: Option<String>,
    /// 展示用地点文字（本期无逆地理编码，取目录名或空）
    pub location: Option<String>,
    /// 标签，以逗号分隔存储
    pub tags: String,
    /// 内容哈希（BLAKE3/xxhash 等，作为去重与缩略图缓存 key）
    pub content_hash: String,
    /// 缩略图缓存绝对路径
    pub thumbnail_path: Option<PathBuf>,
    /// GPS 字段预留（本期暂缓采集与展示）
    pub gps_lon_wgs84: Option<f64>,
    pub gps_lat_wgs84: Option<f64>,
    /// M2 分析结果：模糊评分（Laplacian 方差，未分析时为 None）
    pub blur_score: Option<f64>,
    /// M2 分析结果：是否截图
    pub is_screenshot: bool,
    /// M2 分析结果：感知哈希（用于相似归组）
    pub phash: Option<u64>,
    /// M2 分析结果：所属相似组 id
    pub group_id: Option<i64>,
}

impl Photo {
    /// 展示用尺寸字符串，如 `4032×3024`。
    pub fn size_label(&self) -> String {
        if self.width > 0 && self.height > 0 {
            format!("{}×{}", self.width, self.height)
        } else {
            "—".to_string()
        }
    }

    /// 人类可读的文件大小。
    pub fn file_size_label(&self) -> String {
        let kb = self.size_bytes as f64 / 1024.0;
        if kb >= 1024.0 {
            format!("{:.1} MB", kb / 1024.0)
        } else {
            format!("{:.0} KB", kb)
        }
    }

    /// 展示用日期字符串（无 EXIF 时显示“未知时间”）。
    pub fn date_label(&self) -> String {
        self.date_taken.clone().unwrap_or_else(|| "未知时间".to_string())
    }

    /// 标签列表（按逗号切分并去空白）。
    pub fn tag_list(&self) -> Vec<&str> {
        self.tags.split(',').map(str::trim).filter(|s| !s.is_empty()).collect()
    }

    /// 是否已参加过分析。
    pub fn is_analyzed(&self) -> bool {
        self.blur_score.is_some()
    }
}

/// 相似照片分组（对应 SQLite `photo_groups` 表 + 成员列表）。
#[derive(Clone, Debug)]
pub struct PhotoGroup {
    pub id: i64,
    pub rule: String,
    pub created_at: i64,
    pub members: Vec<Photo>,
}

/// 一个用户选择的扫描根目录。
#[derive(Clone, Debug)]
pub struct ScanRoot {
    pub id: i64,
    pub path: PathBuf,
    pub last_scanned_at: i64,
    /// 该根目录累计索引的照片数
    pub photo_count: i64,
}

/// 从索引路径派生的目录树节点（UI 相册库目录面板使用）。
#[derive(Clone, Debug)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub photo_count: usize,
    pub children: Vec<Folder>,
}

/// 扫描统计结果。
#[derive(Clone, Debug, Default)]
pub struct ScanStats {
    pub photos_found: usize,
    pub photos_added: usize,
    pub photos_updated: usize,
    pub photos_skipped: usize,
    /// 增量清理移除的失效记录数
    pub photos_removed: usize,
    pub errors: usize,
    pub duration_ms: u128,
}

/// 扫描进度事件（后台线程 → UI）。
#[derive(Clone, Debug)]
pub enum ScanProgress {
    Started { root: PathBuf },
    Scanning { processed: usize },
    Finished { stats: ScanStats },
    Failed { message: String },
}

// ===== M3 批量操作与回收站 =====

/// 批量操作类型（对应 `op_log.kind`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpKind {
    /// 删除到回收站
    Trash,
    /// 从回收站恢复
    Restore,
    /// 批量重命名
    Rename,
    /// 清空回收站
    EmptyTrash,
    /// 清理超过保留期的回收站项目
    Purge,
    /// M4 智能归档 · 移动模式
    ArchiveMove,
    /// M4 智能归档 · 复制模式
    ArchiveCopy,
}

impl OpKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpKind::Trash => "trash",
            OpKind::Restore => "restore",
            OpKind::Rename => "rename",
            OpKind::EmptyTrash => "empty_trash",
            OpKind::Purge => "purge",
            OpKind::ArchiveMove => "archive_move",
            OpKind::ArchiveCopy => "archive_copy",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "restore" => OpKind::Restore,
            "rename" => OpKind::Rename,
            "empty_trash" => OpKind::EmptyTrash,
            "purge" => OpKind::Purge,
            "archive_move" => OpKind::ArchiveMove,
            "archive_copy" => OpKind::ArchiveCopy,
            _ => OpKind::Trash,
        }
    }

    /// 中文展示名（首页最近操作、设置页数据概况使用）。
    pub fn label(&self) -> &'static str {
        match self {
            OpKind::Trash => "删除到回收站",
            OpKind::Restore => "从回收站恢复",
            OpKind::Rename => "批量重命名",
            OpKind::EmptyTrash => "清空回收站",
            OpKind::Purge => "清理过期项目",
            OpKind::ArchiveMove => "智能归档（移动）",
            OpKind::ArchiveCopy => "智能归档（复制）",
        }
    }
}

/// 一次批量操作的日志头（对应 `op_log` 表一行）。
#[derive(Clone, Debug)]
pub struct OpLog {
    pub id: i64,
    pub kind: OpKind,
    pub created_at: i64,
    pub item_count: usize,
    pub detail: String,
}

/// 批量操作中的单条路径映射（对应 `op_entries` 表一行）。
/// 重命名：`old_path` → `new_path`；删除：原路径 → 回收站路径。
#[derive(Clone, Debug)]
pub struct OpEntry {
    pub id: i64,
    pub op_id: i64,
    pub old_path: PathBuf,
    pub new_path: PathBuf,
}

/// 回收站项目：保存文件的回收站路径与原始元数据快照，用于完整恢复索引记录。
#[derive(Clone, Debug)]
pub struct TrashItem {
    pub id: i64,
    pub op_id: i64,
    pub original_path: PathBuf,
    pub trash_path: PathBuf,
    pub trashed_at: i64,
    /// 删除前照片记录快照（id 为原照片 id，path 为 original_path）
    pub photo: Photo,
}

/// 删除到回收站的结果汇总。
#[derive(Clone, Debug, Default)]
pub struct TrashResult {
    pub count: usize,
    pub bytes: i64,
    pub op_id: i64,
}

/// 重命名模板引擎的渲染上下文（由一张真实照片派生）。
#[derive(Clone, Debug)]
pub struct RenameContext {
    pub date: String,
    pub time: String,
    pub tag: String,
    pub location: String,
    pub extension: String,
}

/// 重命名预览中的一条：原名 → 新名。
#[derive(Clone, Debug)]
pub struct RenamePreviewItem {
    pub old_name: String,
    pub new_name: String,
}

/// 重命名预览：前若干条样例 + 总数（对话框只展示 3-5 条）。
#[derive(Clone, Debug, Default)]
pub struct RenamePreview {
    pub items: Vec<RenamePreviewItem>,
    pub total: usize,
}

/// 批量重命名执行结果。
#[derive(Clone, Debug, Default)]
pub struct RenameResult {
    pub count: usize,
    pub op_id: i64,
    pub samples: Vec<RenamePreviewItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_kind_roundtrip_and_labels() {
        let kinds = [
            OpKind::Trash,
            OpKind::Restore,
            OpKind::Rename,
            OpKind::EmptyTrash,
            OpKind::Purge,
            OpKind::ArchiveMove,
            OpKind::ArchiveCopy,
        ];
        for k in &kinds {
            assert_eq!(OpKind::from_str(k.as_str()), *k);
            assert!(!k.label().is_empty());
        }
        assert_eq!(OpKind::Trash.label(), "删除到回收站");
        assert_eq!(OpKind::ArchiveCopy.label(), "智能归档（复制）");
    }
}
