# 模块：core-scanner

> 生成时间：2026-11-18 16:58 +0800
> 覆盖范围：src/core/db.rs, src/core/scanner.rs, src/core/thumbnail.rs（按路径排序）
> 覆盖 hash：fec45f251fa6（取前 12 位，由生成者用命令行工具计算）
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
- **M2 分析结果读写**：`pub fn update_analysis(conn, photo_id, blur_score: f64, is_screenshot: bool, phash: Option<u64>) -> Result<()>`；`pub fn save_similar_groups(conn, groups: &[Vec<i64>], created_at: i64) -> Result<()>`（清空旧组重建，自带事务）；`pub fn list_photo_groups(conn) -> Result<Vec<PhotoGroup>>`；`pub fn list_blurry(conn, limit: usize) -> Result<Vec<Photo>>`（按 blur_score 升序）；`pub fn list_screenshots(conn) -> Result<Vec<Photo>>`；`pub fn list_large_files(conn, limit: usize) -> Result<Vec<Photo>>`（按 size_bytes 降序）；`pub fn count_analyzed(conn) -> Result<i64>`。
- **M3 批量操作/回收站读写**：`pub fn get_photo_by_id(conn, id: i64) -> Result<Option<Photo>>`；`pub fn delete_photo_by_path(conn, path: &Path) -> Result<()>`；`pub fn restore_photo(conn, photo: &Photo, root_id: Option<i64>) -> Result<()>`（恢复时重建索引行，ON CONFLICT 更新；M4 归档 Copy 也用它追加副本索引）；`pub fn rename_photo_path(conn, photo_id: i64, new_path: &Path, new_name: &str) -> Result<()>`；操作日志 `insert_op_log(conn, kind: &OpKind, created_at, item_count, detail) -> Result<i64>` / `insert_op_entry(conn, op_id, old: &Path, new: &Path)` / `list_op_entries(conn, op_id) -> Result<Vec<OpEntry>>` / `list_ops(conn, limit) -> Result<Vec<OpLog>>` / `get_op(conn, id) -> Result<Option<OpLog>>` / `delete_op(conn, id)`；回收站 `insert_trash_item(conn, &TrashItem) -> Result<i64>` / `list_trash(conn) -> Result<Vec<TrashItem>>` / `get_trash_item(conn, id)` / `delete_trash_item(conn, id)` / `list_trash_by_op(conn, op_id) -> Result<Vec<i64>>` / `trash_summary(conn) -> Result<(usize, i64)>` / `list_expired_trash(conn, cutoff: i64) -> Result<Vec<TrashItem>>`。
- **M4 settings 读写**：`pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>>`；`pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()>`（INSERT ... ON CONFLICT(key) DO UPDATE，供 UI 持久化归档规则/模式/目标目录）。

**scanner.rs**
- `pub fn scan_root(paths: &Paths, conn: &Connection, root: &Path, progress: Option<&Sender<ScanProgress>>) -> Result<ScanStats>`：扫描一个根目录并写库。

**thumbnail.rs**
- `pub const THUMBNAIL_SIZE: u32 = 400`。
- `pub fn ensure_thumbnail(paths: &Paths, source: &Path, content_hash: &str) -> Result<PathBuf>`：确保缩略图存在并返回缓存路径。

## 内部关键流程
1. `scan_root` → `upsert_scan_root` 取 root_id → `WalkDir` 递归过滤（跳过 `.` 开头目录/trash/node_modules/target，只收图片扩展名）→ 逐张：EXIF（`exif::Reader`，`DateTimeOriginal`/`Make`/`Model`，失败退化为 mtime）→ `twox_hash::XxHash64` 内容哈希 → `ensure_thumbnail` → `image::image_dimensions` 取尺寸 → `upsert_photo`（事务内）→ 上报 `Scanning` 进度。
2. 扫描结束 → `sweep_photos` 删除此根目录下不在本次存活集合的记录 → 提交事务 → 上报 `Finished{stats}`。
3. `upsert_photo` 以 (path, size_bytes, modified_at) 判断是否需要更新，实现增量。
4. schema 含 `scan_roots`/`photos`/`settings` 三表与索引，M2 新增 `photo_groups` 表与 `photos` 的 4 个分析列；M3 新增 `op_log`/`op_entries`/`trash_items` 三表与索引（`op_entries.op_id` 外键级联、`trash_items.trash_path` UNIQUE）；`photos.scan_root_id` 外键级联删除；WAL 模式。
5. M2 迁移分三阶段：①第一个 `execute_batch` 只建表 + 旧列索引；②用 `PRAGMA table_info(photos)` 检查后按需 `ALTER TABLE ADD COLUMN` 补齐 4 个分析列；③**补列之后**再单独 `execute_batch` 建 `idx_photos_blur`/`idx_photos_group`。查询统一用 `PHOTO_COLUMNS` 显式列清单（不用 `SELECT *`）以稳定列顺序。
6. `upsert_photo` 在照片内容变化（size/mtime 变更）时清空blur_score/is_screenshot/phash/group_id，令分析结果失效待重算。

## 依赖关系
- 依赖的外部模块：`rusqlite`（bundled SQLite）、`kamadak-exif`、`walkdir`、`image`、`twox-hash`、`chrono`、`anyhow`；`core-model`（Photo/ScanStats/PhotoGroup/OpLog/OpEntry/TrashItem/OpKind）、`core-config`（Paths）。
- 被谁依赖：`core-analyze`（db 读写分析结果）、`core-ops`（db 读写回收站/操作日志/归档设置）、`ui-desktop`（LibraryData/AnalyzeData/OpsData 通过 db 读数据、AppState 调用 scan_root 与 settings）、集成测试 `tests/scanner_test.rs` / `tests/analyze_test.rs` / `tests/ops_test.rs` / `tests/archive_test.rs`。

## 常见改动点
- 改 schema：在 `migrate` 加 `CREATE TABLE IF NOT EXISTS`/ALTER，同步 `row_to_photo` 列顺序与所有 SQL。
- 加搜索/过滤维度：新增 `list_photos_*` 函数并在 `LibraryData::visible_photos` 或 UI 调用。
- 换缩略图实现（M2 imageflow）：只改 `thumbnail::ensure_thumbnail` 内部，对外接口不变。
- 加图片格式：修改 `IMAGE_EXTS`。

## 已知风险 / 易错点
- **嵌套事务**：`sweep_photos` 不可再开事务，因调用方 `scan_root` 已持有 `unchecked_transaction`；曾因此导致增量清理静默失败。
- `row_to_photo` 用列位置索引，schema 加列必须同步位置，否则数据错位。
- **索引创建顺序**：引用新列的索引必须放在补列之后；曾因把 `idx_photos_blur`/`idx_photos_group` 放进第一个 `execute_batch`，导致 M1 老库（无这些列）打开时报 `no such column` → "迁移 schema 失败"。回归测试 `migrates_legacy_m1_database` 覆盖此场景。
- Windows 上 SQLite 连接未 drop 时临时目录无法删除，测试中需显式 `drop(conn)`。
- M0 遗留的 `delete_scan_root` 会级联删除其下照片记录（外键 ON DELETE CASCADE）。
- HEIC/HEIF 在 `IMAGE_EXTS` 中会被索引元数据，但 `image` 无法解码生成缩略图（本期无 HEIC 支持，属预期）。

## 变更记录
- 2026-11-18 16:58：M4 新增 `get_setting`/`set_setting`（复用 M1 已有 `settings` 表，无 schema 变更）；`restore_photo` 增加归档 Copy 副本索引的调用场景，刷新 hash。
- 2026-11-18 16:05：M3 schema 新增 op_log/op_entries/trash_items 三表与索引；db 新增按 id 查询/删除照片、恢复照片、重命名路径、操作日志与回收站 Repository，刷新 hash。
- 2026-11-18 15:50：修复 M1 老库升级失败——把分析列索引创建移到补列之后，新增老库升级回归测试，刷新 hash。
- 2026-11-18 15:40：M2 新增 photo_groups 表与 4 个分析列的迁移、分析结果读写 Repository、PHOTO_COLUMNS 显式列清单、upsert 时清空旧分析，刷新 hash。
- 2026-11-18 15:10：M1 新建模块，实现 SQLite 索引、递归扫描、EXIF、内容哈希、增量更新与缩略图缓存。
