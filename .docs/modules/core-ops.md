# 模块：core-ops

> 生成时间：2026-11-18 16:05 +0800
> 覆盖范围：src/core/ops/mod.rs, src/core/ops/rename.rs, src/core/ops/trash.rs, tests/ops_test.rs（按路径排序）
> 覆盖 hash：77b5e8c922de（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的批量操作子系统（M3）：回收站（删除/恢复/清空/30 天清理/撤销）与批量重命名模板引擎（预览/执行/冲突处理/撤销），配套操作日志。不依赖 gpui，可独立单元测试。

## 对外接口
**ops/mod.rs**（内部共用）
- `pub(crate) fn move_file(src: &Path, dst: &Path) -> Result<()>`：跨平台移动，先 `rename`，跨盘失败退化为 copy+remove。
- `pub(crate) fn unique_dest(dir: &Path, file_name: &str) -> PathBuf`：目标已存在时追加 `_1`/`_2`…（保留扩展名）。
- `pub(crate) fn sanitize(s: &str) -> String`：清洗文件名非法字符与空白。

**ops/trash.rs**
- `pub fn trash_photos(paths: &Paths, conn: &Connection, photo_ids: &[i64]) -> Result<TrashResult>`：移动文件到 `paths.trash_dir`，写 Trash 操作日志 + op_entries + trash_items，从 photos 移除记录；失败时尝试把已移动文件移回。
- `pub fn restore_items(paths: &Paths, conn: &Connection, trash_ids: &[i64]) -> Result<usize>`：从回收站移回原路径（占用则加后缀），据快照重建索引，删除 trash_item，写 Restore 日志。
- `pub fn empty_trash(paths: &Paths, conn: &Connection) -> Result<usize>`：永久删除回收站副本与记录，写 EmptyTrash 日志。
- `pub fn purge_expired(conn: &Connection, retention_days: i64) -> Result<usize>`：删除超过保留期的项目，写 Purge 日志。
- `pub fn undo_trash(paths: &Paths, conn: &Connection, op_id: i64) -> Result<usize>`：撤销一次删除（还原该 op 的全部 trash_items）。

**ops/rename.rs**
- `pub const PLACEHOLDERS: &[(&str, &str)]`：模板占位符说明表。
- `pub fn context_from_photo(photo: &Photo) -> RenameContext`：从照片派生 date/time/tag/location/extension。
- `pub fn render_stem(pattern: &str, ctx: &RenameContext, seq: usize, original_stem: &str, prefix: &str) -> String`：渲染文件名主干。
- `pub fn preview(conn, photo_ids: &[i64], pattern: &str, prefix: &str, limit: usize) -> Result<RenamePreview>`：预览前 limit 条 + 总数，不落盘；批内重名会消解。
- `pub fn execute(conn, photo_ids: &[i64], pattern: &str, prefix: &str) -> Result<RenameResult>`：执行批量重命名，冲突追加后缀，写 Rename 日志与 op_entries；失败回滚已移动文件。
- `pub fn undo(conn: &Connection, op_id: i64) -> Result<usize>`：依据 op_entries 逆序移回原路径并还原索引，删除该日志。

模板占位符：`{前缀}`/`{prefix}`、`{日期}`/`{date}`（YYYYMMDD）、`{时间}`/`{time}`（HHMMSS）、`{标签}`/`{tag}`、`{地点}`/`{location}`、`{序号}`/`{seq}`、`{序号:3}`/`{seq:3}`（补零宽度）、`{原名}`/`{stem}`、`{扩展名}` 由 context 自动附加。

## 内部关键流程
1. `trash_photos`：create_dir_all(trash_dir) → 逐张生成 `trash_<millis>_<原名>` 唯一路径 → `move_file` → 汇总快照 → 开启事务：insert_op_log(Trash) → 逐条 insert_op_entry(原路径→回收站路径) + insert_trash_item + delete_photo_by_path → 提交。任一文件移动失败则把已移动的移回原路径。
2. `restore_items`：事务内逐条取 trash_item → unique_dest(原目录, 原名) → move_file(回收站→原路径) → 更新 photo 快照 path/file_name → restore_photo 重建索引 → insert_op_entry → delete_trash_item；结束写 Restore 日志。
3. `execute`（重命名）：先全量计算目标名（批内重名序号递增消解）并 `move_file`，再事务写 rename_photo_path + op_entries + Rename 日志；失败时回滚已移动文件。
4. `undo`（重命名）：按 op_entries 把 new_path 移回 old_path（占用加后缀），据当前 path 查回 photo 并还原索引，delete_op。
5. 30 天策略：`purge_expired(conn, 30)` 用 `now_millis - 30*86400_000` 作 cutoff 查询过期项并永久删除其回收站副本。

## 依赖关系
- 依赖的外部模块：`rusqlite`（Connection/事务）、`anyhow`（Result/Context）、`std::fs`；`core-model`（Photo/TrashItem/OpKind/TrashResult/Rename*）、`core-config`（Paths）、`core-db`（Repository）。
- 被谁依赖：`ui-desktop`（AppState 的 batch_trash/batch_rename/undo、重命名页与回收站页）、集成测试 `tests/ops_test.rs`。

## 常见改动点
- 加模板占位符：在 `render_stem` 的 replacements 加键，并同步 `PLACEHOLDERS` 与 ui-desktop 预设模板。
- 改保留期：调整 `purge_expired` 的 retention_days 或 UI 传参。
- 改回收站目录：改 `Paths::trash_dir`（core-config），本模块自动跟随。
- 加新的批量操作类型：扩展 `OpKind` + op_log 写入 + 对应 undo 分支。

## 已知风险 / 易错点
- **不永久删除用户原文件**：普通删除一律 `move_file` 到回收站；只有 `empty_trash`/`purge_expired` 会 `remove_file`，且仅作用于回收站副本。新增删除路径时必须遵守。
- 文件操作不可数据库回滚：各函数采用「先移动文件、失败尽力移回」+「后写库事务」两阶段，移动阶段失败需显式回滚已移动项。
- `move_file` 跨盘 copy+remove 在目标写入成功后才删源，避免数据丢失。
- 恢复/撤销时原路径可能被占用，统一用 `unique_dest` 加后缀，绝不覆盖现有文件。
- `TrashItem.photo` 只快照 M1 基础字段（分析字段置默认），恢复后分析结果失效待重算，属预期。
- 撤销依赖 `op_log`/`op_entries` 未被 `delete_scan_root` 等清理；日志表无自动清理策略（本期保留）。
- 本模块不得引入 gpui 依赖。

## 变更记录
- 2026-11-18 16:05：M3 新建模块，实现回收站、批量重命名模板引擎与撤销，配套集成测试。
