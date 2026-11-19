# 模块：core-ops

> 生成时间：2026-11-19 10:38 +0800
> 覆盖范围：src/core/ops/archive.rs, src/core/ops/mod.rs, src/core/ops/rename.rs, src/core/ops/trash.rs, tests/archive_test.rs, tests/ops_test.rs, tests/settings_test.rs（按路径排序）
> 覆盖 hash：f76508cc0b9d（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的批量操作子系统：回收站（M3）、批量重命名模板引擎（M3）、智能归档规则引擎（M4）、可配置回收站保留期（M5），配套操作日志与撤销。不依赖 gpui，可独立单元测试。2026-11-19 全量审查后强化「文件操作两阶段一致性」：所有批量路径在写库失败时逆序回滚文件，永久删除一律改为「先提交 DB、后删文件」，并补齐目标去重、注入防护与回滚覆盖。

## 对外接口
**ops/mod.rs**（内部共用）
- `pub(crate) fn move_file(src: &Path, dst: &Path) -> Result<()>`：跨平台移动，先 `rename`，跨盘失败退化为 copy+remove；**显式拒绝覆盖已存在的目标**（`dst.exists()` 直接 bail）；copy 成功但删除源失败时清掉刚写的副本，避免孤儿副本。
- `pub(crate) fn unique_dest(dir: &Path, file_name: &str) -> PathBuf`：只接受文件名片段（先 `sanitize` 防路径穿越/绝对路径逃逸），目标已存在时追加 `_1`/`_2`…（保留扩展名）。
- `pub(crate) fn sanitize(s: &str) -> String`：清洗 Windows/Unix 非法字符、控制字符、尾随点/空格与保留设备名（CON/PRN/AUX/NUL/COM1-9/LPT1-9，含扩展名形式）；空串返回 `_`。

**ops/trash.rs**
- `pub fn trash_photos(paths: &Paths, conn: &Connection, photo_ids: &[i64]) -> Result<TrashResult>`：入口对 `photo_ids` 去重；移动文件到 `paths.trash_dir`，写 Trash 操作日志 + op_entries + trash_items，从 photos 移除记录；任一文件移动失败或阶段二写库失败都把已移动文件逆序移回原路径。
- `pub fn restore_items(paths: &Paths, conn: &Connection, trash_ids: &[i64]) -> Result<usize>`：从回收站移回原路径（占用则加后缀），据快照重建索引，删除 trash_item，写 Restore 日志。
- `fn restore_impl(paths, conn, ids, record_log: bool)`（私有）：普通恢复 `record_log=true` 写 Restore 日志；`undo_trash` 用 `false` 不写额外日志。阶段二写库失败时把文件再移回 `trash_path`。
- `pub fn empty_trash(paths: &Paths, conn: &Connection) -> Result<usize>`：永久删除回收站副本与记录，写 EmptyTrash 日志。
- `pub fn purge_expired(conn: &Connection, retention_days: i64) -> Result<usize>`：校验 `RETENTION_MIN_DAYS..=RETENTION_MAX_DAYS`，用 `saturating_mul`/`saturating_sub` 算 cutoff，删除超期项目并写 Purge 日志。
- `fn purge_items(conn, items, kind, detail, now)`（私有）：**先提交 DB 事务（删 trash_items / 写 op_log），再执行 `remove_file`**；文件删除失败只留下孤立文件并记录日志，避免事务回滚导致「文件已删、记录仍在」。`empty_trash` 与 `purge_expired` 共用。
- `pub fn undo_trash(paths: &Paths, conn: &Connection, op_id: i64) -> Result<usize>`：撤销一次删除（还原该 op 的全部 trash_items），仅在 `n > 0` 时删除原 Trash 日志。
- **M5 保留期设置**：`pub const DEFAULT_RETENTION_DAYS: i64 = 30`、`RETENTION_MIN_DAYS: i64 = 1`、`RETENTION_MAX_DAYS: i64 = 3650`、`RETENTION_SETTING_KEY: &str = "trash.retention_days"`；`pub fn retention_days(conn: &Connection) -> i64`（读 settings，未设置/无法解析/越界回退默认 30）；`pub fn set_retention_days(conn: &Connection, days: i64) -> Result<()>`（越界 `anyhow::bail!`，不写库）。

**ops/rename.rs**
- `pub const PLACEHOLDERS: &[(&str, &str)]`：模板占位符说明表（含 `{扩展名}`）。
- `pub fn context_from_photo(photo: &Photo) -> RenameContext`：从照片派生 date/time/tag/location/extension；日期用 `.get(..10)`/`.get(11..19)` 安全切片（防 UTF-8 panic），无 EXIF 时用 `modified_at` 本地 UTC 毫秒格式化兜底。
- `pub fn render_stem(pattern: &str, ctx: &RenameContext, seq: usize, original_stem: &str, prefix: &str) -> String`：渲染文件名主干；`neutral()` 先 `sanitize` 再把花括号替换成 `_`，防止用户前缀/标签/地点里夹带的 `{日期}` 等字面量被二次解析。
- `fn build_plans(conn, photo_ids, pattern, prefix) -> Result<Vec<Plan>>`（私有）：**预览与执行共用的唯一命名计算入口**；对 `photo_ids` 去重，用 `occupied: HashSet<PathBuf>` + `self_path` 统一消解批内冲突、磁盘已有文件与原地不动，模板无 `{序号}` 也能追加 `_1/_2`，循环有 1_000_000 防御上限。
- `pub fn preview(conn, photo_ids: &[i64], pattern: &str, prefix: &str, limit: usize) -> Result<RenamePreview>`：预览前 limit 条 + 总数，不落盘。
- `pub fn execute(conn, photo_ids: &[i64], pattern: &str, prefix: &str) -> Result<RenameResult>`：执行批量重命名，写 Rename 日志与 op_entries；阶段一移动文件失败或阶段二写库失败都会逆序把文件移回原名。
- `pub fn undo(conn: &Connection, op_id: i64) -> Result<usize>`：依据 op_entries 逆序移回原路径并还原索引，删除该日志；写库失败时把文件移回 `new_path`。

模板占位符：`{前缀}`/`{prefix}`、`{日期}`/`{date}`（YYYYMMDD）、`{时间}`/`{time}`（HHMMSS）、`{标签}`/`{tag}`、`{地点}`/`{location}`、`{序号}`/`{seq}`、`{序号:3}`/`{seq:3}`（补零宽度，最大 9）、`{原名}`/`{stem}`、`{扩展名}`/`{ext}`（原扩展名，不含点）。

**ops/archive.rs**（M4）
- `pub enum ArchiveRule`：`ByYearMonth`（默认）/`ByYear`/`ByStory`/`ByTag`；`pub const ALL: &'static [ArchiveRule]`；`label() -> &'static str`、`example() -> &'static str`、`as_str() -> &'static str`、`from_str(s: &str) -> Self`。
- `pub enum ArchiveMode`：`Move`（默认）/`Copy`；`label()`、`as_str()`、`from_str()`。
- `pub struct ArchiveItem`：`photo_id/from_path/to_path/relative/size_bytes/conflict` 均 `pub`。
- `pub struct ArchiveTreeNode`：`name/rel_path/dir_count/file_count/total_count/total_bytes/files/sample/children` 均 `pub`。
- `pub struct ArchivePlan`：`rule/mode/dest_root/items/tree/total_files/total_bytes/conflict_count` 均 `pub`，`#[derive(Clone, Debug)]`。
- `pub struct ArchiveResult`：`count/op_id/dest_root` 均 `pub`，`Default`。
- `pub fn plan(conn: &Connection, photo_ids: &[i64], rule: ArchiveRule, mode: ArchiveMode, dest_root: &Path) -> Result<ArchivePlan>`：只读生成文件映射与完整目录树预览；**目标必须绝对路径**，且**不能位于照片目录内部**；跳过 `photo.path.starts_with(dest_root)` 的照片（防止 Copy 副本被反复归档）；目标重名追加 `_1` 后缀，不创建目录/不移动文件/不写日志。`date_parts` 用 `chrono::NaiveDate::parse_from_str` 校验，异常日期返回 `未知日期 / 未知月份`。
- `pub fn execute(conn: &Connection, plan: &ArchivePlan) -> Result<ArchiveResult>`：严格使用 `item.to_path` 落盘（不再执行时静默改路径，保证预览目录树与落盘一致）；Move=`move_file`，Copy=`fs::copy` + 事务写 `ArchiveMove`/`ArchiveCopy` 日志与 op_entries；文件阶段失败回滚 Move 文件 / 删除 Copy 副本，阶段二写库失败同样回滚文件。
- `pub fn undo(conn: &Connection, op_id: i64) -> Result<usize>`：`ArchiveMove` 把文件移回原路径并还原索引（写库失败移回 `new_path`）；`ArchiveCopy` **先提交 DB（删副本索引与 op），再永久删除副本文件**，删除失败只记录日志。

## 内部关键流程
1. `trash_photos`：入口去重 → create_dir_all(trash_dir) → 逐张生成唯一路径 → `move_file` → 汇总快照 → 开启事务：insert_op_log(Trash) → 逐条 insert_op_entry + insert_trash_item + delete_photo_by_path → 提交；任一阶段失败逆序把已移动文件移回原路径。
2. `restore_items` / `undo_trash` 共用 `restore_impl(..., record_log)`：取 trash_item → `unique_dest`(原目录, 原名) → `move_file`(回收站→原路径) → 更新快照 → `restore_photo` 重建索引 → `delete_trash_item`；阶段二写库失败把文件再移回 `trash_path`。普通恢复额外写 Restore 日志，撤销删除不写。
3. `empty_trash` / `purge_expired` 共用 `purge_items`：**先提交 DB 事务**（删记录 + 写日志），**再 `remove_file`** 回收站副本；文件删除失败不回滚 DB，只留下孤立文件。
4. `execute`（重命名）：`build_plans()` 一次算出全部目标名（批内重名 / 磁盘占用用 `occupied` 消解）→ 逐张 `move_file` → 事务写 rename_photo_path + op_entries + Rename 日志；任一阶段失败逆序回滚文件。`preview` 复用同一 `build_plans()`，结果严格一致。
5. `undo`（重命名）：按 op_entries 逆序把 new_path 移回 old_path（用 `allocate_dest` 防占用），据当前 path 查回 photo 并还原索引，delete_op。
6. 保留期策略（M5 起可配置）：`purge_expired(conn, retention_days)` 先校验 1..=3650，再用 `now_millis.saturating_sub(days.saturating_mul(86400_000))` 作 cutoff 查询过期项（`trashed_at < cutoff`）并永久删除其回收站副本；天数由 `retention_days(conn)` 从 settings 读取（默认 30）。
7. **归档计划**（`plan`）：校验 `dest_root.is_absolute()` 与「目标不在照片目录内」→ 跳过 dest_root 内已有副本 → `date_parts` 派生 (年, 年-月)，缺失用「未知日期 / 未知月份」；`story_label` 取地点→首标签→「未命名故事」；`tag_label` 取首标签→「未分类」；`dest_components` 按规则生成目录分段（`sanitize` 后过滤空串/`.`/`..`）；`plan_unique` 用 `HashSet<PathBuf>` + `exists()` 消解重名；`build_tree` 把 `relative`（`/` 分隔）插入 `BTreeMap` 树，递归 `to_node` 汇总计数。
8. **归档执行与撤销**：`execute` 按 `plan.to_path` 落盘 → 事务写日志 / op_entries / 索引；`ArchiveMove::undo` 移回文件并还原索引；`ArchiveCopy::undo` 先提交 DB 再删副本文件（源文件与源索引不变）。

## 依赖关系
- 依赖的外部模块：`rusqlite`（Connection/事务）、`anyhow`（Result/Context）、`chrono`（日期校验）、`std::fs`；`core-model`（Photo/TrashItem/OpKind/TrashResult/Rename*）、`core-config`（Paths）、`core-db`（Repository）。
- 被谁依赖：`ui-desktop`（AppState 的 batch_trash/batch_rename/undo、重命名/归档/回收站/设置页）、集成测试 `tests/ops_test.rs` / `tests/archive_test.rs` / `tests/settings_test.rs`。

## 常见改动点
- 加模板占位符：在 `render_stem` 的 replacements 加键，并同步 `PLACEHOLDERS` 与 ui-desktop 预设模板；替换值必须经过 `neutral()`。
- 改保留期：调 `set_retention_days`（键 `trash.retention_days`）；`retention_days` 对脏值一律回退默认，`purge_expired` 的入参由 UI 从该值传入，勿再硬编码 30。
- 加归档规则：扩展 `ArchiveRule` 与 `dest_components`，同步 `ALL`/`label`/`example`/`as_str`/`from_str` 与 UI 规则卡片（自动遍历 `ALL`）。
- 改目录树预览展示：改 `ArchiveTreeNode` 字段与 `to_node` 聚合逻辑，UI 侧 `flatten_archive_node` 跟随。
- 加新的批量操作类型：扩展 `OpKind` + op_log 写入 + 对应 undo 分支，并遵守「先落盘、失败回滚、后写库」两阶段。

## 已知风险 / 易错点
- **不永久删除用户原文件**：普通删除一律 `move_file` 到回收站；只有 `empty_trash`/`purge_expired`/`ArchiveCopy::undo` 会 `remove_file`，且仅作用于回收站副本 / 本次生成的副本，绝不触碰源文件。
- **文件操作不可数据库回滚**：采用「先移动/复制文件、失败尽力逆序回滚」+「后写库事务」两阶段；阶段二失败已移动文件必须移回，否则索引指向旧路径。
- **永久删除必须在事务提交之后**：`purge_items` / `ArchiveCopy::undo` 若在事务内 `remove_file`，事务回滚会造成「文件已不可逆删除但记录仍在」。文件删除失败只留孤立文件，不再尝试回滚 DB。
- `move_file` 显式拒绝覆盖目标；跨盘 copy+remove 在目标写入成功后才删源，删源失败则清理副本。
- 恢复/撤销/归档时目标路径可能被占用，统一用 `unique_dest`（预览用 `plan_unique` / `allocate_dest`）加后缀，绝不覆盖现有文件。
- **预览与落盘一致性**：`preview` 与 `execute` 必须共用 `build_plans()`；`execute`（归档）必须消费 `plan()` 产出的 `to_path`；UI 执行前会重新 `refresh_archive_plan()`，避免用过期计划。
- **模板无序号**：`allocate_dest` 用 `occupied` 集合而非依赖序号递增消解冲突，否则同名模板会死循环；替换值必须 `neutral()` 防占位符注入。
- `TrashItem.photo` 只快照 M1 基础字段（分析字段置默认），恢复后分析结果失效待重算，属预期；归档 Copy 追加的副本索引同理不继承分析结果。
- 归档目标若落在扫描根目录内或照片目录内，会被 `plan` 直接拒绝；Copy 模式位于 `dest_root` 内的副本会被跳过，避免副本增殖。
- 撤销依赖 `op_log`/`op_entries` 未被 `delete_scan_root` 等清理；日志表无自动清理策略（本期保留）。
- 本模块不得引入 gpui 依赖。

## 变更记录
- 2026-11-19 10:38：全量 bug 审查修复——`move_file` 拒绝覆盖与孤儿副本清理、`sanitize` 补控制字符/保留设备名、三处批量路径补齐两阶段回滚、永久删除改「先提交 DB 后删文件」、重命名 `build_plans()` 统一预览/执行与注入防护、归档目标校验与副本跳过、保留期越界校验与 saturating 运算；新增 4 个 `ops/mod.rs` 单测 + 4 个 ops 集成回归 + 2 个归档回归；刷新 hash。
- 2026-11-18 17:40：M5 新增可配置回收站保留期（常量 + `retention_days`/`set_retention_days`，复用 settings 表）与 `tests/settings_test.rs`（默认值/往返/脏值回退/越界拒绝），刷新覆盖范围与 hash。
- 2026-11-18 16:58：M4 新增 `src/core/ops/archive.rs`（4 种规则 / Move+Copy / 目录树预览 / 执行 / 撤销）与 `tests/archive_test.rs`；`OpKind` 新增 `ArchiveMove`/`ArchiveCopy`；`db` 新增 settings 读写；整模块重写并刷新 hash。
- 2026-11-18 16:05：M3 新建模块，实现回收站、批量重命名模板引擎与撤销，配套集成测试。
