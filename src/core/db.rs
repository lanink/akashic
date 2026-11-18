//! SQLite 索引层：schema、迁移与 Repository。
//!
//! 不依赖 gpui，可独立单元测试。所有写操作走事务，支撑后续批量操作的可撤销性。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::core::model::{Photo, ScanRoot};

/// 打开（或创建）索引库并执行迁移。
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("打开索引库失败: {}", path.display()))?;
    migrate(&conn)?;
    Ok(conn)
}

/// 在内存中打开（用于测试）。
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS scan_roots (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            path            TEXT NOT NULL UNIQUE,
            last_scanned_at INTEGER NOT NULL DEFAULT 0,
            photo_count     INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS photos (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            path            TEXT NOT NULL UNIQUE,
            file_name       TEXT NOT NULL,
            size_bytes      INTEGER NOT NULL,
            date_taken      TEXT,
            modified_at     INTEGER NOT NULL,
            width           INTEGER NOT NULL DEFAULT 0,
            height          INTEGER NOT NULL DEFAULT 0,
            device          TEXT,
            location        TEXT,
            tags            TEXT NOT NULL DEFAULT '',
            content_hash    TEXT NOT NULL DEFAULT '',
            thumbnail_path  TEXT,
            gps_lon_wgs84   REAL,
            gps_lat_wgs84   REAL,
            scan_root_id    INTEGER REFERENCES scan_roots(id) ON DELETE CASCADE,
            blur_score      REAL,
            is_screenshot   INTEGER NOT NULL DEFAULT 0,
            phash           INTEGER,
            group_id        INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_photos_path ON photos(path);
        CREATE INDEX IF NOT EXISTS idx_photos_hash ON photos(content_hash);
        CREATE INDEX IF NOT EXISTS idx_photos_date ON photos(date_taken);
        CREATE INDEX IF NOT EXISTS idx_photos_root ON photos(scan_root_id);
        CREATE INDEX IF NOT EXISTS idx_photos_blur ON photos(blur_score);
        CREATE INDEX IF NOT EXISTS idx_photos_group ON photos(group_id);

        CREATE TABLE IF NOT EXISTS photo_groups (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            rule            TEXT NOT NULL DEFAULT 'phash+dssim',
            created_at      INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )
    .context("迁移 schema 失败")?;

    // M1 老库迁移：新库在建表时已含这些列，老库按需补列，避免重复 ALTER 报错。
    {
        let cols: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(photos)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?
        };
        let add = |name: &str, ddl: &str| -> Result<()> {
            if !cols.contains(name) {
                conn.execute(ddl, [])?;
            }
            Ok(())
        };
        add("blur_score", "ALTER TABLE photos ADD COLUMN blur_score REAL")?;
        add("is_screenshot", "ALTER TABLE photos ADD COLUMN is_screenshot INTEGER NOT NULL DEFAULT 0")?;
        add("phash", "ALTER TABLE photos ADD COLUMN phash INTEGER")?;
        add("group_id", "ALTER TABLE photos ADD COLUMN group_id INTEGER")?;
    }
    Ok(())
}

/// photos 表的稳定列顺序（避免 `SELECT *` 因迁移加列导致位置映射错位）。
const PHOTO_COLUMNS: &str = "id, path, file_name, size_bytes, date_taken, modified_at, \
     width, height, device, location, tags, content_hash, thumbnail_path, \
     gps_lon_wgs84, gps_lat_wgs84, blur_score, is_screenshot, phash, group_id";

// ===== scan_roots =====

/// 登记或更新一个扫描根目录，返回其 id。
pub fn upsert_scan_root(conn: &Connection, root_path: &Path) -> Result<i64> {
    let now = now_millis();
    conn.execute(
        "INSERT INTO scan_roots (path, last_scanned_at) VALUES (?1, ?2)
         ON CONFLICT(path) DO UPDATE SET last_scanned_at = excluded.last_scanned_at",
        params![root_path.to_string_lossy(), now],
    )?;
    conn.query_row(
        "SELECT id FROM scan_roots WHERE path = ?1",
        params![root_path.to_string_lossy()],
        |row| row.get(0),
    )
    .context("查询 scan_root id 失败")
}

pub fn list_scan_roots(conn: &Connection) -> Result<Vec<ScanRoot>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, r.path, r.last_scanned_at,
                (SELECT COUNT(*) FROM photos p WHERE p.scan_root_id = r.id) AS photo_count
         FROM scan_roots r ORDER BY r.path",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ScanRoot {
            id: row.get(0)?,
            path: PathBuf::from(row.get::<_, String>(1)?),
            last_scanned_at: row.get(2)?,
            photo_count: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>().context("列出扫描根目录失败")
}

pub fn delete_scan_root(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM scan_roots WHERE id = ?1", params![id])?;
    Ok(())
}

// ===== photos =====

/// 按路径查询单张照片。
pub fn get_photo_by_path(conn: &Connection, path: &Path) -> Result<Option<Photo>> {
    conn.query_row(
        &format!("SELECT {PHOTO_COLUMNS} FROM photos WHERE path = ?1"),
        params![path.to_string_lossy()],
        row_to_photo,
    )
    .optional()
    .context("查询照片失败")
}

/// 插入或更新一张照片（基于路径 + 大小 + 修改时间判断是否需要更新）。
/// 返回 true 表示新增，false 表示跳过（未变化）。
pub fn upsert_photo(conn: &Connection, photo: &Photo, root_id: i64) -> Result<bool> {
    let path_str = photo.path.to_string_lossy();
    let existing: Option<(i64, i64, i64)> = conn
        .query_row(
            "SELECT id, size_bytes, modified_at FROM photos WHERE path = ?1",
            params![path_str],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;

    let changed = match existing {
        Some((_, size, modified)) => size != photo.size_bytes || modified != photo.modified_at,
        None => true,
    };

    if !changed {
        return Ok(false);
    }

    if let Some((id, _, _)) = existing {
        conn.execute(
            "UPDATE photos SET file_name=?2, size_bytes=?3, date_taken=?4, modified_at=?5,
                    width=?6, height=?7, device=?8, location=?9, tags=?10, content_hash=?11,
                    thumbnail_path=?12, gps_lon_wgs84=?13, gps_lat_wgs84=?14, scan_root_id=?15,
                    blur_score=NULL, is_screenshot=0, phash=NULL, group_id=NULL
             WHERE id=?1",
            params![
                id,
                photo.file_name,
                photo.size_bytes,
                photo.date_taken,
                photo.modified_at,
                photo.width,
                photo.height,
                photo.device,
                photo.location,
                photo.tags,
                photo.content_hash,
                photo.thumbnail_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                photo.gps_lon_wgs84,
                photo.gps_lat_wgs84,
                root_id,
            ],
        )?;
    } else {
        conn.execute(
            "INSERT INTO photos (path, file_name, size_bytes, date_taken, modified_at,
                    width, height, device, location, tags, content_hash, thumbnail_path,
                    gps_lon_wgs84, gps_lat_wgs84, scan_root_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                path_str,
                photo.file_name,
                photo.size_bytes,
                photo.date_taken,
                photo.modified_at,
                photo.width,
                photo.height,
                photo.device,
                photo.location,
                photo.tags,
                photo.content_hash,
                photo.thumbnail_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                photo.gps_lon_wgs84,
                photo.gps_lat_wgs84,
                root_id,
            ],
        )?;
    }
    Ok(true)
}

/// 列出所有照片（按拍摄时间倒序）。
pub fn list_photos(conn: &Connection) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(&format!("SELECT {PHOTO_COLUMNS} FROM photos ORDER BY date_taken DESC, id DESC"))?;
    let rows = stmt.query_map([], row_to_photo)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().context("列出照片失败")
}

/// 列出指定扫描根目录下（路径前缀匹配）的照片。
pub fn list_photos_under(conn: &Connection, root: &Path) -> Result<Vec<Photo>> {
    let prefix = format!("{}%", root.to_string_lossy());
    let mut stmt = conn.prepare(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE path LIKE ?1 ORDER BY date_taken DESC, id DESC"
    ))?;
    let rows = stmt.query_map(params![prefix], row_to_photo)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().context("按目录列出照片失败")
}

/// 按关键词搜索文件名 / 地点 / 标签。
pub fn search_photos(conn: &Connection, keyword: &str) -> Result<Vec<Photo>> {
    let pattern = format!("%{}%", keyword);
    let mut stmt = conn.prepare(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos
         WHERE file_name LIKE ?1 OR location LIKE ?1 OR tags LIKE ?1
         ORDER BY date_taken DESC, id DESC"
    ))?;
    let rows = stmt.query_map(params![pattern], row_to_photo)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().context("搜索照片失败")
}

/// 更新缩略图路径。
pub fn set_thumbnail(conn: &Connection, photo_id: i64, thumb: &Path) -> Result<()> {
    conn.execute(
        "UPDATE photos SET thumbnail_path = ?2 WHERE id = ?1",
        params![photo_id, thumb.to_string_lossy()],
    )?;
    Ok(())
}

pub fn count_photos(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM photos", [], |row| row.get(0))
        .context("统计照片数失败")
}

pub fn total_size_bytes(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COALESCE(SUM(size_bytes),0) FROM photos", [], |row| row.get(0))
        .context("统计占用空间失败")
}

// ===== 分析结果（M2） =====

/// 写入单张照片的分析结果。phash 为 u64，按位存为 INTEGER，读出时转回 u64。
pub fn update_analysis(
    conn: &Connection,
    photo_id: i64,
    blur_score: f64,
    is_screenshot: bool,
    phash: Option<u64>,
) -> Result<()> {
    conn.execute(
        "UPDATE photos SET blur_score=?2, is_screenshot=?3, phash=?4 WHERE id=?1",
        params![photo_id, blur_score, is_screenshot as i64, phash.map(|h| h as i64)],
    )?;
    Ok(())
}

/// 保存相似分组：清空旧组后重建（分析是全量重算）。
pub fn save_similar_groups(conn: &Connection, groups: &[Vec<i64>], created_at: i64) -> Result<()> {
    let tx = conn.unchecked_transaction().context("开启分组写入事务失败")?;
    tx.execute("UPDATE photos SET group_id = NULL", [])?;
    tx.execute("DELETE FROM photo_groups", [])?;
    for members in groups {
        if members.len() < 2 {
            continue;
        }
        tx.execute(
            "INSERT INTO photo_groups (rule, created_at) VALUES ('phash+dssim', ?1)",
            params![created_at],
        )?;
        let group_id = tx.last_insert_rowid();
        for id in members {
            tx.execute("UPDATE photos SET group_id = ?2 WHERE id = ?1", params![id, group_id])?;
        }
    }
    tx.commit().context("提交分组写入失败")?;
    Ok(())
}

/// 列出所有相似组及其成员照片。
pub fn list_photo_groups(conn: &Connection) -> Result<Vec<crate::core::model::PhotoGroup>> {
    use crate::core::model::PhotoGroup;
    let mut stmt = conn.prepare(
        "SELECT id, rule, created_at FROM photo_groups ORDER BY created_at DESC, id DESC",
    )?;
    let headers = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut groups = Vec::new();
    for (id, rule, created_at) in headers {
        let mut mstmt = conn.prepare(&format!(
            "SELECT {PHOTO_COLUMNS} FROM photos WHERE group_id = ?1 ORDER BY id"
        ))?;
        let members = mstmt
            .query_map(params![id], row_to_photo)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        groups.push(PhotoGroup { id, rule, created_at, members });
    }
    Ok(groups)
}

/// 按模糊分升序返回最模糊的 N 张（仅已分析照片）。
pub fn list_blurry(conn: &Connection, limit: usize) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE blur_score IS NOT NULL \
         ORDER BY blur_score ASC LIMIT ?1"
    ))?;
    stmt.query_map(params![limit as i64], row_to_photo)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("查询模糊照片失败")
}

/// 列出所有被判定为截图的照片。
pub fn list_screenshots(conn: &Connection) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos WHERE is_screenshot = 1 \
         ORDER BY date_taken DESC, id DESC"
    ))?;
    stmt.query_map([], row_to_photo)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("查询截图失败")
}

/// 按文件大小降序返回前 N 张大文件。
pub fn list_large_files(conn: &Connection, limit: usize) -> Result<Vec<Photo>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PHOTO_COLUMNS} FROM photos ORDER BY size_bytes DESC LIMIT ?1"
    ))?;
    stmt.query_map(params![limit as i64], row_to_photo)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("查询大文件失败")
}

/// 统计已分析照片数。
pub fn count_analyzed(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM photos WHERE blur_score IS NOT NULL", [], |row| row.get(0))
        .context("统计已分析照片数失败")
}

/// 增量扫描清理：删除指定扫描根目录下、不在本次存活路径集合中的照片记录。
/// `root` 为扫描根目录，`alive_paths` 为本次扫描实际找到的绝对路径。
pub fn sweep_photos(conn: &Connection, root: &Path, alive_paths: &[PathBuf]) -> Result<usize> {
    let existing = list_photos_under(conn, root)?;
    let alive: std::collections::HashSet<&Path> = alive_paths.iter().map(|p| p.as_path()).collect();
    let mut removed = 0;
    // 不在此处开启事务：调用方（scanner）已持有事务，嵌套事务会失败。
    // 单独调用时每条 DELETE 各自自动提交。
    for photo in existing {
        if !alive.contains(photo.path.as_path()) {
            conn.execute("DELETE FROM photos WHERE id = ?1", params![photo.id])?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn row_to_photo(row: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id: row.get(0)?,
        path: PathBuf::from(row.get::<_, String>(1)?),
        file_name: row.get(2)?,
        size_bytes: row.get(3)?,
        date_taken: row.get(4)?,
        modified_at: row.get(5)?,
        width: row.get(6)?,
        height: row.get(7)?,
        device: row.get(8)?,
        location: row.get(9)?,
        tags: row.get(10)?,
        content_hash: row.get(11)?,
        thumbnail_path: row.get::<_, Option<String>>(12)?.map(PathBuf::from),
        gps_lon_wgs84: row.get(13)?,
        gps_lat_wgs84: row.get(14)?,
        blur_score: row.get(15)?,
        is_screenshot: row.get::<_, i64>(16)? != 0,
        phash: row.get::<_, Option<i64>>(17)?.map(|v| v as u64),
        group_id: row.get(18)?,
    })
}

/// 当前 Unix 毫秒时间戳。
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_photo(path: &str, size: i64, modified: i64) -> Photo {
        Photo {
            id: 0,
            path: PathBuf::from(path),
            file_name: Path::new(path).file_name().unwrap().to_string_lossy().to_string(),
            size_bytes: size,
            date_taken: Some("2024-08-15 09:45:00".to_string()),
            modified_at: modified,
            width: 4032,
            height: 3024,
            device: Some("TestCam".to_string()),
            location: None,
            tags: "旅行,风景".to_string(),
            content_hash: "abc123".to_string(),
            thumbnail_path: None,
            gps_lon_wgs84: None,
            gps_lat_wgs84: None,
            blur_score: None,
            is_screenshot: false,
            phash: None,
            group_id: None,
        }
    }

    #[test]
    fn schema_migrates_and_crud() -> Result<()> {
        let conn = open_in_memory()?;
        let root = upsert_scan_root(&conn, Path::new("/photos/trip"))?;
        assert!(root > 0);

        let p = sample_photo("/photos/trip/IMG_0001.jpg", 1000, 100);
        assert!(upsert_photo(&conn, &p, root)?); // 新增
        assert_eq!(count_photos(&conn)?, 1);

        // 未变化则跳过
        assert!(!upsert_photo(&conn, &p, root)?);

        // 修改后更新
        let p2 = sample_photo("/photos/trip/IMG_0001.jpg", 2000, 200);
        assert!(upsert_photo(&conn, &p2, root)?);
        assert_eq!(count_photos(&conn)?, 1);
        let fetched = get_photo_by_path(&conn, Path::new("/photos/trip/IMG_0001.jpg"))?.unwrap();
        assert_eq!(fetched.size_bytes, 2000);

        // 查询
        assert_eq!(list_photos(&conn)?.len(), 1);
        assert_eq!(list_photos_under(&conn, Path::new("/photos/trip"))?.len(), 1);
        assert_eq!(search_photos(&conn, "IMG_0001")?.len(), 1);
        assert_eq!(search_photos(&conn, "旅行")?.len(), 1);

        // 再插入另一张以验证搜索区分
        let p3 = sample_photo("/photos/trip/DSCN_0002.jpg", 3000, 300);
        upsert_photo(&conn, &p3, root)?;
        assert_eq!(count_photos(&conn)?, 2);
        assert_eq!(search_photos(&conn, "DSCN")?.len(), 1);

        // 删除根目录级联清理照片
        delete_scan_root(&conn, root)?;
        assert_eq!(count_photos(&conn)?, 0);
        Ok(())
    }

    #[test]
    fn photo_labels() {
        let p = sample_photo("/photos/trip/IMG_0001.jpg", 2 * 1024 * 1024, 100);
        assert_eq!(p.size_label(), "4032×3024");
        assert_eq!(p.file_size_label(), "2.0 MB");
        assert_eq!(p.date_label(), "2024-08-15 09:45:00");
        assert_eq!(p.tag_list(), vec!["旅行", "风景"]);
    }
}
