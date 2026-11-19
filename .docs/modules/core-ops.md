# 模块：core-ops

> 生成时间：2026-11-18 17:40 +0800
> 覆盖范围：src/core/ops/archive.rs, src/core/ops/mod.rs, src/core/ops/rename.rs, src/core/ops/trash.rs, tests/archive_test.rs, tests/ops_test.rs, tests/settings_test.rs（按路径排序）
> 覆盖 hash：c46895ffcc9d（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的批量操作子系统：回收站（M3）、批量重命名模板引擎（M3）、智能归档规则引擎（M4）、可配置回收站保留期（M5），配套操作日志与撤销。不依赖 gpui，可独立单元测试。

## 对外接口
**ops/mod.rs**（内部共用）
- `pub(crate) fn move_file(src: &Path, dst: &Path) -> Result<()>`：跨平台移动，先 `rename`，跨盘失败退化为 copy+remove。
- `pub(crate) fn unique_dest(dir: &Path, file_name: &str) -> PathBuf`：目标已存在时追加 `_1`/`_2`…（保留扩展名）。
- `pub(crate) fn sanitize(s: &str) -> String`：清洗文件名非法字符与空白（归档目录分段复用）。

**ops/trash.rs**
- `pub fn trash_photos(paths: &Paths, conn: &Connection, photo_ids: &[i64]) -> Result<TrashResult>`：移动文件到 `paths.trash_dir`，写 Trash 操作日志 + op_entries + trash_items，从 photos 移除记录；失败时尝试把已移动文件移回。
- `pub fn restore_items(paths: &Paths, conn: &Connection, trash_ids: &[i64]) -> Result<usize>`：从回收站移回原路径（占用则加后缀），据快照重建索引，删除 trash_item，写 Restore 日志。
- `pub fn empty_trash(paths: &Paths, conn: &Connection) -> Result<usize>`：永久删除回收站副本与记录，写 EmptyTrash 日志。
- `pub fn purge_expired(conn: &Connection, retention_days: i64) -> Result<usize>`：删除超过保留期的项目，写 Purge 日志。
- `pub fn undo_trash(paths: &Paths, conn: &Connection, op_id: i64) -> Result<usize>`：撤销一次删除（还原该 op 的全部 trash_items）。
- **M5 保留期设置**：`pub const DEFAULT_RETENTION_DAYS: i64 = 30`、`RETENTION_MIN_DAYS: i64 = 1`、`RETENTION_MAX_DAYS: i64 = 3650`、`RETENTION_SETTING_KEY: &str = "trash.retention_days"`；`pub fn retention_days(conn: &Connection) -> i64`（读 settings，未设置/无法解析/越界回退默认 30）；`pub fn set_retention_days(conn: &Connection, days: i64) -> Result<()>`（越界 `anyhow::bail!`，不写库）。

**ops/rename.rs**
- `pub const PLACEHOLDERS: &[(&str, &str)]`：模板占位符说明表。
- `pub fn context_from_photo(photo: &Photo) -> RenameContext`：从照片派生 date/time/tag/location/extension。
- `pub fn render_stem(pattern: &str, ctx: &RenameContext, seq: usize, original_stem: &str, prefix: &str) -> String`：渲染文件名主干。
- `pub fn preview(conn, photo_ids: &[i64], pattern: &str, prefix: &str, limit: usize) -> Result<RenamePreview>`：预览前 limit 条 + 总数，不落盘；批内重名会消解。
- `pub fn execute(conn, photo_ids: &[i64], pattern: &str, prefix: &str) -> Result<RenameResult>`：执行批量重命名，冲突追加后缀，写 Rename 日志与 op_entries；失败回滚已移动文件。
- `pub fn undo(conn: &Connection, op_id: i64) -> Result<usize>`：依据 op_entries 逆序移回原路径并还原索引，删除该日志。

模板占位符：`{前缀}`/`{prefix}`、`{日期}`/`{date}`（YYYYMMDD）、`{时间}`/`{time}`（HHMMSS）、`{标签}`/`{tag}`、`{地点}`/`{location}`、`{序号}`/`{seq}`、`{序号:3}`/`{seq:3}`（补零宽度）、`{原名}`/`{stem}`、`{扩展名}` 由 context 自动附加。

**ops/archive.rs**（M4）
- `pub enum ArchiveRule`：`ByYearMonth`（默认）/`ByYear`/`ByStory`/`ByTag`；`pub const ALL: &'static [ArchiveRule]`；`label() -> &'static str`、`example() -> &'static str`、`as_str() -> &'static str`、`from_str(s: &str) -> Self`。
- `pub enum ArchiveMode`：`Move`（默认）/`Copy`；`label()`、`as_str()`、`from_str()`。
- `pub struct ArchiveItem`：`photo_id/from_path/to_path/relative/size_bytes/conflict` 均 `pub`。
- `pub struct ArchiveTreeNode`：`name/rel_path/dir_count/file_count/total_count/total_bytes/files/sample/children` 均 `pub`。
- `pub struct ArchivePlan`：`rule/mode/dest_root/items/tree/total_files/total_bytes/conflict_count` 均 `pub`，`#[derive(Clone, Debug)]`。
- `pub struct ArchiveResult`：`count/op_id/dest_root` 均 `pub`，`Default`。
- `pub fn plan(conn: &Connection, photo_ids: &[i64], rule: ArchiveRule, mode: ArchiveMode, dest_root: &Path) -> Result<ArchivePlan>`：只读生成文件映射与完整目录树预览，目标重名追加 `_1` 后缀，不创建目录/不移动文件/不写日志。
- `pub fn execute(conn: &Connection, plan: &ArchivePlan) -> Result<ArchiveResult>`：两阶段落盘（Move=`move_file`，Copy=`fs::copy`）+ 事务写 `ArchiveMove`/`ArchiveCopy` 日志、op_entries；Move 更新原索引 path，Copy 用 `restore_photo` 追加副本索引；失败回滚已处理项。
- `pub fn undo(conn: &Connection, op_id: i64) -> Result<usize>`：按日志 kind 分支——Move 把文件移回原路径并还原索引；Copy 删除本次生成的副本文件与副本索引（源文件与源索引不变）。

## 内部关键流程
1. `trash_photos`：create_dir_all(trash_dir) → 逐张生成 `trash_<millis>_<原名>` 唯一路径 → `move_file` → 汇总快照 → 开启事务：insert_op_log(Trash) → 逐条 insert_op_entry(原路径→回收站路径) + insert_trash_item + delete_photo_by_path → 提交。任一文件移动失败则把已移动的移回原路径。
2. `restore_items`：事务内逐条取 trash_item → unique_dest(原目录, 原名) → move_file(回收站→原路径) → 更新 photo 快照 path/file_name → restore_photo 重建索引 → insert_op_entry → delete_trash_item；结束写 Restore 日志。
3. `execute`（重命名）：先全量计算目标名（批内重名序号递增消解）并 `move_file`，再事务写 rename_photo_path + op_entries + Rename 日志；失败时回滚已移动文件。
4. `undo`（重命名）：按 op_entries 把 new_path 移回 old_path（占用加后缀），据当前 path 查回 photo 并还原索引，delete_op。
5. 保留期策略（M5 起可配置）：`purge_expired(conn, retention_days)` 用 `now_millis - retention_days*86400_000` 作 cutoff 查询过期项并永久删除其回收站副本；天数由 `retention_days(conn)` 从 settings 读取（默认 30），UI 设置页通过 `set_retention_days` 写入并校验 1..=3650。
6. **归档计划**（`plan`）：`date_parts` 从 `date_taken` 派生 (年, 年-月)，缺失用「未知日期」；`story_label` 取地点→首标签→「未命名故事」；`tag_label` 取首标签→「未分类」；`dest_components` 按规则生成目录分段（`sanitize` 后过滤空串/`.`/`..`，全空回退「未分类」）；`plan_unique` 用 `HashSet<PathBuf>` + `exists()` 消解批内与盘上重名；`build_tree` 把 `relative`（`/` 分隔）插入 `BTreeMap` 树，递归 `to_node` 汇总 file_count/total_count/total_bytes/sample。
7. **归档执行**（`execute`）：逐项按 `plan.to_path` 落盘（若预览后目标被占用再用 `unique_dest` 兜底）；事务内 `insert_op_log(ArchiveMove/ArchiveCopy)` → 每项 `insert_op_entry(src→dst)`；Move 调 `rename_photo_path`，Copy 克隆 photo（id=0、path=dst）调 `restore_photo` 追加副本索引。文件阶段失败：Move 把已移动项移回，Copy 删除已生成副本。
8. **归档撤销**（`undo`）：`ArchiveMove` 逐条把 new_path 移回 old_path（unique_dest 防占用）并按 new_path 查回 photo 还原索引；`ArchiveCopy` 逐条 `remove_file(new_path)`（副本，非用户原文件）+ `delete_photo_by_path(new_path)`；最后 `delete_op` 删除日志。

## 依赖关系
- 依赖的外部模块：`rusqlite`（Connection/事务）、`anyhow`（Result/Context）、`std::fs`；`core-model`（Photo/TrashItem/OpKind/TrashResult/Rename*）、`core-config`（Paths）、`core-db`（Repository）。
- 被谁依赖：`ui-desktop`（AppState 的 batch_trash/batch_rename/undo、重命名/归档/回收站/设置页）、集成测试 `tests/ops_test.rs` / `tests/archive_test.rs` / `tests/settings_test.rs`。

## 常见改动点
- 加模板占位符：在 `render_stem` 的 replacements 加键，并同步 `PLACEHOLDERS` 与 ui-desktop 预设模板。
- 改保留期：调 `set_retention_days`（键 `trash.retention_days`）；`retention_days` 对脏值一律回退默认，`purge_expired` 的入参由 UI 从该值传入，勿再硬编码 30。
- 加归档规则：扩展 `ArchiveRule` 与 `dest_components`，同步 `ALL`/`label`/`example`/`as_str`/`from_str` 与 UI 规则卡片（自动遍历 `ALL`）。
- 改目录树预览展示：改 `ArchiveTreeNode` 字段与 `to_node` 聚合逻辑，UI 侧 `flatten_archive_node` 跟随。
- 加新的批量操作类型：扩展 `OpKind` + op_log 写入 + 对应 undo 分支。

## 已知风险 / 易错点
- **不永久删除用户原文件**：普通删除一律 `move_file` 到回收站；只有 `empty_trash`/`purge_expired` 会 `remove_file`，且仅作用于回收站副本。`ArchiveCopy::undo` 删除的是本次生成的归档副本，且强制 `new_path != old_path`，绝不触碰源文件。
- 文件操作不可数据库回滚：各函数采用「先移动/复制文件、失败尽力回滚」+「后写库事务」两阶段。
- `move_file` 跨盘 copy+remove 在目标写入成功后才删源，避免数据丢失。
- 恢复/撤销/归档时目标路径可能被占用，统一用 `unique_dest`（预览用 `plan_unique`）加后缀，绝不覆盖现有文件。
- **预览与落盘一致性**：`execute` 必须消费 `plan()` 产出的 `to_path`；UI 执行前会重新 `refresh_archive_plan()`，避免用过期计划。
- `TrashItem.photo` 只快照 M1 基础字段（分析字段置默认），恢复后分析结果失效待重算，属预期；归档 Copy 追加的副本索引同理不继承分析结果。
- 撤销依赖 `op_log`/`op_entries` 未被 `delete_scan_root` 等清理；日志表无自动清理策略（本期保留）。
- 归档目标若落在扫描根目录内，下次扫描会把归档副本当新照片重复索引；UI 默认目标为「扫描根目录的同级 / 归档」以规避。
- 本模块不得引入 gpui 依赖。

## 变更记录
- 2026-11-18 17:40：M5 新增可配置回收站保留期（常量 + `retention_days`/`set_retention_days`，复用 settings 表）与 `tests/settings_test.rs`（默认值/往返/脏值回退/越界拒绝），刷新覆盖范围与 hash。
- 2026-11-18 16:58：M4 新增 `src/core/ops/archive.rs`（4 种规则 / Move+Copy / 目录树预览 / 执行 / 撤销）与 `tests/archive_test.rs`；`OpKind` 新增 `ArchiveMove`/`ArchiveCopy`；`db` 新增 settings 读写；整模块重写并刷新 hash。
- 2026-11-18 16:05：M3 新建模块，实现回收站、批量重命名模板引擎与撤销，配套集成测试。
