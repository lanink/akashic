# 模块：core-scanner

> 生成时间：2026-11-19 10:38 +0800
> 覆盖范围：src/core/db.rs, src/core/scanner.rs, src/core/thumbnail.rs, tests/perf_test.rs, tests/scanner_test.rs（按路径排序）
> 覆盖 hash：0e97b65fe29f（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的索引与扫描子系统：SQLite schema 与 Repository、walkdir 递归扫描 + EXIF + 内容哈希 + 增量更新、缩略图缓存。不依赖 gpui，可在后台线程运行并独立测试；M6 新增 5000 张性能基准（独立 `#[ignore]` 测试）。2026-11-19 全量审查后修复：按 `scan_root_id` 精确 sweep、LIKE 通配符转义、遍历出错阻止清理、内容哈希纳入变化判断、EXIF 日期校验与本地时区回退、缩略图原子写与 0 字节缓存重建。

## 对外接口
**db.rs**
- `pub fn open(path: &Path) -> Result<Connection>`：打开/创建索引库并迁移（三阶段），设置 `busy_timeout(Duration::from_secs(10))`。
- `pub fn open_in_memory() -> Result<Connection>`。
- `pub fn upsert_scan_root(conn, root_path: &Path) -> Result<i64>`。
- `pub fn list_scan_roots(conn) -> Result<Vec<ScanRoot>>`、`pub fn delete_scan_root(conn, id: i64) -> Result<()>`。
- `pub fn get_photo_by_path(conn, path: &Path) -> Result<Option<Photo>>`、`pub fn get_photo_by_id(conn, id: i64) -> Result<Option<Photo>>`。
- `pub enum UpsertOutcome { Added, Updated, Skipped }`（`Clone/Copy/Debug/PartialEq/Eq`）。
- `pub fn upsert_photo(conn, photo: &Photo, root_id: i64) -> Result<UpsertOutcome>`：比较 `(size_bytes, modified_at, content_hash)` 三元组；内容哈希变化但 size/mtime 不变也会刷新索引并清空分析字段（避免缩略图缓存 key 与旧结果沿用）。
- `pub fn list_photos(conn) -> Result<Vec<Photo>>`、`pub fn list_photos_under(conn, root: &Path) -> Result<Vec<Photo>>`（`escape_like` + `ESCAPE '\'`，目录名含 `%`/`_` 不越界）、`pub fn search_photos(conn, keyword: &str) -> Result<Vec<Photo>>`。
- `pub fn set_thumbnail(conn, photo_id, thumb: &Path) -> Result<()>`、`pub fn count_photos(conn) -> Result<i64>`、`pub fn total_size_bytes(conn) -> Result<i64>`。
- `pub fn sweep_photos(conn, root_id: i64, alive_paths: &[PathBuf]) -> Result<usize>`：按 `scan_root_id` 精确取候选，路径比较统一用 `to_string_lossy()`；**不自行开启事务**，由调用方（scanner）负责。
- `pub fn now_millis() -> i64`。
- **M2 分析结果读写**：`pub fn update_analysis(conn, photo_id, blur_score: f64, is_screenshot: bool, phash: Option<u64>) -> Result<()>`；`pub fn save_similar_groups(conn, groups: &[Vec<i64>], created_at: i64) -> Result<()>`（清空旧组重建，自带事务）；`pub fn list_photo_groups(conn) -> Result<Vec<PhotoGroup>>`（过滤成员 <2 的空/单人组）；`pub fn list_blurry(conn, limit: usize) -> Result<Vec<Photo>>`（只返回 `blur_score < BLUR_THRESHOLD`，`blur_score ASC, id ASC`）；`pub fn list_screenshots(conn) -> Result<Vec<Photo>>`；`pub fn list_large_files(conn, limit: usize) -> Result<Vec<Photo>>`；`pub fn count_analyzed(conn) -> Result<i64>`。
- **M3 批量操作/回收站读写**：`pub fn delete_photo_by_path(conn, path: &Path) -> Result<()>`；`pub fn restore_photo(conn, photo: &Photo, root_id: Option<i64>) -> Result<()>`（恢复时重建索引行，ON CONFLICT 更新；M4 归档 Copy 也用它追加副本索引）；`pub fn rename_photo_path(conn, photo_id: i64, new_path: &Path, new_name: &str) -> Result<()>`；操作日志 `insert_op_log(conn, kind: &OpKind, created_at, item_count, detail) -> Result<i64>` / `insert_op_entry` / `list_op_entries` / `list_ops` / `get_op` / `delete_op`；回收站 `insert_trash_item` / `list_trash` / `get_trash_item` / `delete_trash_item` / `list_trash_by_op` / `trash_summary` / `list_expired_trash(conn, cutoff: i64)`（`trashed_at < cutoff`，严格小于）。
- **M4 settings 读写**：`pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>>`；`pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()>`。

**scanner.rs**
- `pub fn scan_root(paths: &Paths, conn: &Connection, root: &Path, progress: Option<&Sender<ScanProgress>>) -> Result<ScanStats>`：扫描一个根目录并写库；`ScanStats.photos_added/photos_updated/photos_skipped` 由 `UpsertOutcome` 三态统计，进度用 `PROGRESS_EVERY = 32` 节流。

**thumbnail.rs**
- `pub const THUMBNAIL_SIZE: u32 = 400`。
- `pub fn ensure_thumbnail(paths: &Paths, source: &Path, content_hash: &str) -> Result<PathBuf>`：先 `create_dir_all(cache_dir)`；缓存必须存在且 `len() > 0`（0 字节残片重写）；哈希为空时用「路径 + 大小 + mtime」退化 key 并加 `fb_` 前缀隔离命名空间；临时文件写入后 `rename`（写前清理 0 字节残片）原子提交。

**tests/perf_test.rs（M6 性能基准，默认忽略）**
- `#[test] #[ignore] fn bench_5000_photos()`：临时目录生成 5000 张 16×16 合成 PNG（颜色随序号变化，保证内容哈希互不相同），分 20 个子目录；`AKASHIC_BENCH_COUNT` 可缩小规模且解析后 `.max(1)` 防除零；测 `scanner::scan_root`、`LibraryData::reload`、`visible_photos()` ×100 耗时并用 `--nocapture` 打印；不测 O(n²) 的相似分析。

## 内部关键流程
1. `scan_root` → `upsert_scan_root` 取 root_id → `WalkDir` 递归过滤（跳过 `.` 开头目录 / trash / node_modules / target，以及应用自身 `data_dir/cache_dir/trash_dir`；只收图片扩展名，**HEIC/HEIF 已移除**；符号链接形式的图片用 `std::fs::metadata` 跟随判断）→ 逐张：EXIF（`exif::Reader`，`DateTimeOriginal`/`Make`/`Model`，`normalize_exif_date` 校验合法性拒绝 `0000:00:00`/乱码；失败用 `with_timezone(&chrono::Local)` 的 mtime 回退）→ `twox_hash::XxHash64` 内容哈希 → `ensure_thumbnail` → `image::image_dimensions` 取尺寸 → `upsert_photo`（事务内）→ 每 32 张上报一次 `Scanning` 进度。
2. 扫描结束：**仅当 `walk_errors == 0`** 才调用 `sweep_photos(&tx, root_id, alive_paths)` 删除此根下不在存活集合的记录（遍历错误被吞时清理会误删索引）→ 提交事务 → 上报 `Finished{stats}`。
3. `upsert_photo` 以 (path, size_bytes, modified_at, content_hash) 判断是否需要更新，返回 `Added/Updated/Skipped` 三态；更新时清空 blur_score/is_screenshot/phash/group_id，令分析结果失效待重算。
4. schema 含 `scan_roots`/`photos`/`settings` 三表与索引，M2 新增 `photo_groups` 表与 `photos` 的 4 个分析列；M3 新增 `op_log`/`op_entries`/`trash_items` 三表与索引（`op_entries.op_id` 外键级联、`trash_items.trash_path` UNIQUE）；`photos.scan_root_id` 外键级联删除；WAL 模式。
5. M2 迁移分三阶段：①第一个 `execute_batch` 只建表 + 旧列索引；②用 `PRAGMA table_info(photos)` 检查后按需 `ALTER TABLE ADD COLUMN` 补齐 4 个分析列；③**补列之后**再单独 `execute_batch` 建 `idx_photos_blur`/`idx_photos_group`。查询统一用 `PHOTO_COLUMNS` 显式列清单（不用 `SELECT *`）以稳定列顺序。
6. M6 基准流程：`tests/perf_test.rs` 先生成合成图（不计入指标）→ `scan_root` 写库 → `LibraryData::reload` 冷启动 → `visible_photos()` 多次过滤；打印各段耗时与每张均值，断言索引数量与筛选结果，最后 `drop(conn)` 再清理临时目录。

## 依赖关系
- 依赖的外部模块：`rusqlite`（bundled SQLite）、`kamadak-exif`、`walkdir`、`image`、`twox-hash`、`chrono`、`anyhow`；`core-model`（Photo/ScanStats/PhotoGroup/OpLog/OpEntry/TrashItem/OpKind/BLUR_THRESHOLD）、`core-config`（Paths）。
- 被谁依赖：`core-analyze`（db 读写分析结果）、`core-ops`（db 读写回收站/操作日志/归档设置）、`ui-desktop`（LibraryData/AnalyzeData/OpsData 通过 db 读数据、AppState 调用 scan_root 与 settings）、集成测试 `tests/scanner_test.rs` / `tests/analyze_test.rs` / `tests/ops_test.rs` / `tests/archive_test.rs`；M6 性能基准 `tests/perf_test.rs` 直接调用 `scan_root` 与 `LibraryData::reload`。

## 常见改动点
- 改 schema：在 `migrate` 加 `CREATE TABLE IF NOT EXISTS`/ALTER，同步 `row_to_photo` 列顺序与所有 SQL。
- 加搜索/过滤维度：新增 `list_photos_*` 函数并在 `LibraryData::visible_photos` 或 UI 调用；含路径前缀的 LIKE 必须走 `escape_like`。
- 换缩略图实现（M2 imageflow）：只改 `thumbnail::ensure_thumbnail` 内部，对外接口不变。
- 加图片格式：修改 `IMAGE_EXTS`（须确认 `image` crate 能解码，否则不要入索引）。

## 已知风险 / 易错点
- **嵌套事务**：`sweep_photos` 不可再开事务，因调用方 `scan_root` 已持有 `unchecked_transaction`；曾因此导致增量清理静默失败。
- **遍历错误阻止清理**：`WalkDir` 出错必须计数，只要 `walk_errors > 0` 就跳过 `sweep`，否则一次权限/IO 错误会把整库索引清空。
- **LIKE 通配符**：目录名含 `%` / `_`（如 `2024_旅行`）时前缀 LIKE 会越界匹配；`list_photos_under` 已用 `escape_like` + `ESCAPE '\'`。
- **sweep 按 root_id**：旧实现按路径前缀 LIKE 清理，前缀重叠/大小写差异会误删别的扫描根记录；现按 `scan_root_id` 精确取候选。
- `row_to_photo` 用列位置索引，schema 加列必须同步位置，否则数据错位。
- **索引创建顺序**：引用新列的索引必须放在补列之后；曾因把 `idx_photos_blur`/`idx_photos_group` 放进第一个 `execute_batch`，导致 M1 老库（无这些列）打开时报 `no such column` → "迁移 schema 失败"。回归测试 `migrates_legacy_m1_database` 覆盖此场景。
- **缩略图缓存**：0 字节文件是崩溃残片必须重写；写入用临时文件 + `rename` 原子提交；退化 key 加 `fb_` 前缀，避免与正常内容哈希命名空间互相覆盖。
- Windows 上 SQLite 连接未 drop 时临时目录无法删除，测试中需显式 `drop(conn)`。
- M0 遗留的 `delete_scan_root` 会级联删除其下照片记录（外键 ON DELETE CASCADE）。
- HEIC/HEIF 已从 `IMAGE_EXTS` 移除：本期无解码能力，不再入索引（原实现会入索引但无法生成缩略图）。

## 变更记录
- 2026-11-19 10:38：全量 bug 审查修复（`busy_timeout`、`UpsertOutcome` 三态与内容哈希纳入变化判断、`sweep_photos` 按 root_id、LIKE 转义、遍历错误阻止 sweep、`list_blurry` 阈值与稳定排序、`list_photo_groups` 过滤空组、`list_expired_trash` 严格小于、EXIF 日期校验与本地时区、移除 HEIC/HEIF、排除应用自身目录、进度节流、缩略图原子写/0 字节/退化 key 隔离、perf 规模 `.max(1)`）；新增 3 个 db 单测 + 4 个 scanner 集成回归；刷新 hash。
- 2026-11-19：M6 新增 `tests/perf_test.rs`（5000 张扫描/冷启动/筛选基准，`#[ignore]`）；覆盖范围纳入 `tests/perf_test.rs` 与 `tests/scanner_test.rs` 并刷新 hash。
- 2026-11-18 16:58：M4 新增 `get_setting`/`set_setting`（复用 M1 已有 `settings` 表，无 schema 变更）；`restore_photo` 增加归档 Copy 副本索引的调用场景，刷新 hash。
- 2026-11-18 16:05：M3 schema 新增 op_log/op_entries/trash_items 三表与索引；db 新增按 id 查询/删除照片、恢复照片、重命名路径、操作日志与回收站 Repository，刷新 hash。
- 2026-11-18 15:50：修复 M1 老库升级失败——把分析列索引创建移到补列之后，新增老库升级回归测试，刷新 hash。
- 2026-11-18 15:40：M2 新增 photo_groups 表与 4 个分析列的迁移、分析结果读写 Repository、PHOTO_COLUMNS 显式列清单、upsert 时清空旧分析，刷新 hash。
- 2026-11-18 15:10：M1 新建模块，实现 SQLite 索引、递归扫描、EXIF、内容哈希、增量更新与缩略图缓存。
