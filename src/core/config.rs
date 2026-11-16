//! 配置与路径：统一提供应用数据、缓存、回收站目录。
//!
//! 纯 std + dirs 实现，不依赖 gpui，可独立单元测试。
//! 所有路径都在本机应用数据目录下，符合“纯本地、零上传”约束。

use std::path::PathBuf;

/// 应用标识，用于定位平台应用数据目录。
pub const APP_ID: &str = "akashic";

/// Akashic 的全部本地路径配置。
#[derive(Clone, Debug)]
pub struct Paths {
    /// 应用数据根目录（索引库、日志、配置）。
    pub data_dir: PathBuf,
    /// 缩略图缓存目录。
    pub cache_dir: PathBuf,
    /// 回收站目录：所有删除先移动到这里，30 天后清理。
    pub trash_dir: PathBuf,
    /// SQLite 索引库文件。
    pub db_path: PathBuf,
    /// 日志文件。
    pub log_path: PathBuf,
}

impl Paths {
    /// 基于本机应用数据目录构造全部路径（目录尚未创建）。
    pub fn from_app_dir(base: PathBuf) -> Self {
        let root = base.join(APP_ID);
        Self {
            db_path: root.join("index.sqlite3"),
            log_path: root.join("akashic.log"),
            cache_dir: root.join("cache").join("thumbnails"),
            trash_dir: root.join("trash"),
            data_dir: root,
        }
    }

    /// 使用平台默认应用数据目录。
    pub fn platform_default() -> Option<Self> {
        dirs::data_local_dir().map(Self::from_app_dir)
    }

    /// 创建全部目录（幂等）。
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.trash_dir)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_derive_from_base() {
        let p = Paths::from_app_dir(PathBuf::from("/tmp/base"));
        assert_eq!(p.data_dir, PathBuf::from("/tmp/base/akashic"));
        assert_eq!(p.db_path, PathBuf::from("/tmp/base/akashic/index.sqlite3"));
        assert_eq!(p.log_path, PathBuf::from("/tmp/base/akashic/akashic.log"));
        assert_eq!(p.cache_dir, PathBuf::from("/tmp/base/akashic/cache/thumbnails"));
        assert_eq!(p.trash_dir, PathBuf::from("/tmp/base/akashic/trash"));
    }
}
