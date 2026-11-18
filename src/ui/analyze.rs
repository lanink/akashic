//! UI 分析数据适配层：把 core 的分析结果（相似组/模糊/截图/大文件）
//! 转换成整理页可直接消费的视图数据。依赖 core，不直接访问文件系统。

use crate::core::db;
use crate::core::model::{Photo, PhotoGroup};
use rusqlite::Connection;

/// 整理页当前展示的分析结果快照。
#[derive(Clone, Default)]
pub struct AnalyzeData {
    pub groups: Vec<PhotoGroup>,
    pub blurry: Vec<Photo>,
    pub screenshots: Vec<Photo>,
    pub large: Vec<Photo>,
    pub analyzed_count: i64,
    pub loaded: bool,
}

impl AnalyzeData {
    /// 从数据库重新加载全部分析结果。
    pub fn reload(conn: &Connection) -> anyhow::Result<Self> {
        Ok(Self {
            groups: db::list_photo_groups(conn)?,
            blurry: db::list_blurry(conn, 50)?,
            screenshots: db::list_screenshots(conn)?,
            large: db::list_large_files(conn, 50)?,
            analyzed_count: db::count_analyzed(conn)?,
            loaded: true,
        })
    }

    /// 各工具的徽标数字。
    pub fn badges(&self) -> [String; 4] {
        [
            format!("{}组", self.groups.len()),
            format!("{}张", self.blurry.len()),
            format!("{}张", self.screenshots.len()),
            format!("{}个", self.large.len()),
        ]
    }

    /// 是否已有分析结果可展示。
    pub fn is_ready(&self) -> bool {
        self.loaded && self.analyzed_count > 0
    }
}
