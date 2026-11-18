# 模块：core-model

> 生成时间：2026-11-18 15:40 +0800
> 覆盖范围：src/core/config.rs, src/core/mod.rs, src/core/model.rs（按路径排序）
> 覆盖 hash：d4a4d0c6ed02（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的数据模型与本地路径配置，不依赖 gpui，可独立单元测试。`Photo` 在 M1 真实字段基础上，M2 新增 `blur_score/is_screenshot/phash/group_id` 四个分析结果字段，并新增 `PhotoGroup` 相似分组模型；M0 模拟数据仍保留待清理。

## 对外接口
- `pub struct Photo`（src/core/model.rs）：字段 `id/path/file_name/size_bytes/date_taken/modified_at/width/height/device/location/tags/content_hash/thumbnail_path/gps_lon_wgs84/gps_lat_wgs84/blur_score/is_screenshot/phash/group_id` 均 `pub`。M2 新增：`blur_score: Option<f64>`、`is_screenshot: bool`、`phash: Option<u64>`、`group_id: Option<i64>`。
- `Photo` 方法：`size_label()`、`file_size_label()`、`date_label()`、`tag_list()`、`is_analyzed() -> bool`（blur_score 有值即为已分析）。
- `pub struct PhotoGroup`（M2）：`id/rule/created_at/members: Vec<Photo>`，对应 `photo_groups` 表 + 成员。
- `pub struct ScanRoot`：`id/path/last_scanned_at/photo_count`。
- `pub struct Folder`：`id/name/path/photo_count/children`，目录树节点。
- `pub struct ScanStats`：`photos_found/photos_added/photos_updated/photos_skipped/photos_removed/errors/duration_ms`。
- `pub enum ScanProgress`：`Started{root}` / `Scanning{processed}` / `Finished{stats}` / `Failed{message}`。
- `pub const APP_ID: &str = "akashic"`、`pub struct Paths`（src/core/config.rs，字段 `data_dir/cache_dir/trash_dir/db_path/log_path`）。
- `Paths` 方法：`from_app_dir(base: PathBuf) -> Self`、`platform_default() -> Option<Self>`、`ensure_dirs() -> std::io::Result<()>`。
- M0 遗留：`PhotoDef`、`PHOTOS`、`FolderNode`、`folder_defs()`、`expandable_id()`（UI 已不引用）。

## 内部关键流程
1. scanner 扫描后构造 `Photo`（分析字段为默认 None/false），`db::upsert_photo` 写入索引；UI 通过 `LibraryData::reload` 读回。
2. M2 分析后 `db::update_analysis` 回填 `blur_score/is_screenshot/phash`，`db::save_similar_groups` 回填 `group_id` 并建 `PhotoGroup`。
3. `Photo` 的 `*_label` 方法把原始字段转成 UI 展示文本（尺寸、大小、日期、标签）。
4. 扫描进度用 `ScanProgress`、分析进度用 `AnalyzeProgress`（见 core-analyze）经 channel 从后台线程发给 UI。
5. 组合根调用 `Paths::platform_default/ensure_dirs` 确保本地目录存在。

## 依赖关系
- 依赖的外部模块：`dirs`（应用数据目录）、`std::fs`/`std::path`。
- 被谁依赖：`core-scanner`（构造 Photo/ScanStats）、`core-analyze`（PhotoAnalysis/PhotoGroup）、`ui-desktop`（LibraryData/AnalyzeData 消费 Photo/Folder/PhotoGroup）、`main.rs`（Paths）。

## 常见改动点
- 新增照片字段：在 `Photo` 加字段 → 同步 `db.rs` 的 schema、`PHOTO_COLUMNS`、`row_to_photo`、`upsert_photo` 与所有构造处（scanner/analyze 测试）。
- 调整展示格式：改 `Photo::*_label` 方法。
- 新增进度类型：扩展 `ScanProgress` 或 `AnalyzeProgress` 并同步 UI 轮询处理。
- 清理 M0 模拟数据：确认 UI 无引用后删除 `PhotoDef/PHOTOS/FolderNode/folder_defs/expandable_id`。

## 已知风险 / 易错点
- `dirs` 锁定 4.0.0（与 Cargo.lock 一致），改 6.x 会触发离线解析失败。
- `Photo` 字段新增若漏改 `PHOTO_COLUMNS`/`row_to_photo` 的列索引，会导致数据错位且难以察觉。
- gps_* 字段本期预留但不采集（地图功能暂缓），不要误填。
- 本模块不得引入 gpui 依赖。

## 变更记录
- 2026-11-18 15:40：M2 为 Photo 新增 4 个分析字段与 `is_analyzed()`，新增 `PhotoGroup`，刷新 hash。
- 2026-11-18 15:10：M1 引入真实数据模型（Photo/ScanRoot/Folder/ScanStats/ScanProgress），新增 `photos_removed` 统计；整模块重写。
- 2026-11-16 17:35：M0 新建 core 层，迁入模拟数据与路径配置。
