# 模块：core-scanner

> 生成时间：2026-11-18 15:10 +0800
> 覆盖范围：src/core/db.rs, src/core/scanner.rs, src/core/thumbnail.rs（按路径排序）
> 覆盖 hash：808d37dcd3f9（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的索引与扫描子系统：SQLite schema 与 Repository、walkdir 递归扫描 + EXIF + 内容哈希 + 增量更新、缩略图缓存。不依赖 gpui，可在后台线程运行并独立测试。

## 对外接口
**db.rs**
- `pub fn open(path: &Path) -> Result<Connection>`：打开/创建索引库并迁移。
- `pub fn upsert_scan_root(conn, root_path: &Path) -> Result<i64>`。
- `pub fn list_scan_roots(conn) -> Result<Vec<ScanRoot>>`、`pub fn delete_scan_root(conn, id: i64) -> Result<()>`。
- `pub fn get_photo_by_path(conn, path: &Path) -> Result<Option<Photo>>`。
- `pub fn upsert_photo(conn, photo: &Photo, root_id: i64) -> Result<bool>`：返回 true=新增/更新，false=未变化跳过。
- `pub fn list_photos(conn) -> Result<Vec<Photo>>`、`pub fn list_photos_under(conn, root: &Path) -> Result<Vec<Photo>>`、`pub fn search_photos(conn, keyword: &str) -> Result<Vec<Photo>>`。
- `pub fn set_thumbnail(conn, photo_id, thumb: &Path) -> Result<()>`、`pub fn count_photos(conn) -> Result<i64>`、`pub fn total_size_bytes(conn) -> Result<i64>`。
- `pub fn sweep_photos(conn, root: &Path, alive_paths: &[PathBuf]) -> Result<usize>`：增量清理失效记录。**注意：不自行开启事务**，由调用方负责。
- `pub fn now_millis() -> i64`。

**scanner.rs**
- `pub fn scan_root(paths: &Paths, conn: &Connection, root: &Path, progress: Option<&Sender<ScanProgress>>) -> Result<ScanStats>`：扫描一个根目录并写库。

**thumbnail.rs**
- `pub const THUMBNAIL_SIZE: u32 = 400`。
- `pub fn ensure_thumbnail(paths: &Paths, source: &Path, content_hash: &str) -> Result<PathBuf>`：确保缩略图存在并返回缓存路径。

## 内部关键流程
1. `scan_root` → `upsert_scan_root` 取 root_id → `WalkDir` 递归过滤（跳过 `.` 开头目录/trash/node_modules/target，只收图片扩展名）→ 逐张：EXIF（`exif::Reader`，`DateTimeOriginal`/`Make`/`Model`，失败退化为 mtime）→ `twox_hash::XxHash64` 内容哈希 → `ensure_thumbnail` → `image::image_dimensions` 取尺寸 → `upsert_photo`（事务内）→ 上报 `Scanning` 进度。
2. 扫描结束 → `sweep_photos` 删除此根目录下不在本次存活集合的记录 → 提交事务 → 上报 `Finished{stats}`。
3. `upsert_photo` 以 (path, size_bytes, modified_at) 判断是否需要更新，实现增量。
4. schema 含 `scan_roots`/`photos`/`settings` 三表与索引；`photos.scan_root_id` 外键级联删除；WAL 模式。

## 依赖关系
- 依赖的外部模块：`rusqlite`（bundled SQLite）、`kamadak-exif`、`walkdir`、`image`、`twox-hash`、`chrono`、`anyhow`；`core-model`（Photo/ScanStats）、`core-config`（Paths）。
- 被谁依赖：`ui-desktop`（LibraryData 通过 db 读数据、AppState 调用 scan_root）、集成测试 `tests/scanner_test.rs`。

## 常见改动点
- 改 schema：在 `migrate` 加 `CREATE TABLE IF NOT EXISTS`/ALTER，同步 `row_to_photo` 列顺序与所有 SQL。
- 加搜索/过滤维度：新增 `list_photos_*` 函数并在 `LibraryData::visible_photos` 或 UI 调用。
- 换缩略图实现（M2 imageflow）：只改 `thumbnail::ensure_thumbnail` 内部，对外接口不变。
- 加图片格式：修改 `IMAGE_EXTS`。

## 已知风险 / 易错点
- **嵌套事务**：`sweep_photos` 不可再开事务，因调用方 `scan_root` 已持有 `unchecked_transaction`；曾因此导致增量清理静默失败。
- `row_to_photo` 用列位置索引，schema 加列必须同步位置，否则数据错位。
- Windows 上 SQLite 连接未 drop 时临时目录无法删除，测试中需显式 `drop(conn)`。
- M0 遗留的 `delete_scan_root` 会级联删除其下照片记录（外键 ON DELETE CASCADE）。
- HEIC/HEIF 在 `IMAGE_EXTS` 中会被索引元数据，但 `image` 无法解码生成缩略图（本期无 HEIC 支持，属预期）。

## 变更记录
- 2026-11-18 15:10：M1 新建模块，实现 SQLite 索引、递归扫描、EXIF、内容哈希、增量更新与缩略图缓存。
