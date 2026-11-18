//! UI 批量操作数据适配层：把 core 的回收站/操作日志转换成页面可直接消费的快照。
//! 依赖 core，不直接访问文件系统。

use rusqlite::Connection;

use crate::core::db;
use crate::core::model::{OpLog, TrashItem};

/// 回收站页面快照。
#[derive(Clone, Default)]
pub struct OpsData {
    pub trash: Vec<TrashItem>,
    pub trash_count: usize,
    pub trash_bytes: i64,
    pub recent_ops: Vec<OpLog>,
    pub loaded: bool,
}

impl OpsData {
    pub fn reload(conn: &Connection) -> anyhow::Result<Self> {
        let trash = db::list_trash(conn)?;
        let (trash_count, trash_bytes) = db::trash_summary(conn)?;
        let recent_ops = db::list_ops(conn, 20)?;
        Ok(Self {
            trash,
            trash_count,
            trash_bytes,
            recent_ops,
            loaded: true,
        })
    }

    /// 人类可读的回收站占用空间。
    pub fn size_label(&self) -> String {
        let kb = self.trash_bytes as f64 / 1024.0;
        if kb >= 1024.0 {
            format!("{:.1} MB", kb / 1024.0)
        } else {
            format!("{:.0} KB", kb)
        }
    }
}
