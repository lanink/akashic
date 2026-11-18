# 模块：ui-desktop

> 生成时间：2026-11-18 15:10 +0800
> 覆盖范围：src/ui/app.rs, src/ui/assets.rs, src/ui/library.rs, src/ui/mod.rs（按路径排序）
> 覆盖 hash：a777486cc90b（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面 UI 层。M1 起相册库/首页从本地 SQLite 索引读取真实数据，支持原生目录选择、后台扫描、目录过滤、多选与照片详情；智能整理/重命名/回收站仍为占位。

## 对外接口
- `pub struct AppState`：`page/paths/data/active_folder_id/folder_expanded/selected/scan_status/is_scanning/progress_rx`（字段私有）。
- `pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState>`：创建初始状态并尝试从索引加载。
- `AppState::visible_count() -> usize`、`AppState::start_scan(root: PathBuf, cx: &mut Context<Self>)`：启动后台扫描。
- `pub struct AppRoot` + `pub fn AppRoot::new(app: Entity<AppState>) -> Self` + `impl Render`。
- `pub struct Assets` + `impl AssetSource`（src/ui/assets.rs，编译期嵌入 22 个图标）。
- `LibraryData`（src/ui/library.rs）：`reload(conn) -> Result<Self>`、`visible_photos() -> Vec<&Photo>`、`thumbnail_for(&Photo) -> Option<PathBuf>`；字段 `photos/folders/active_folder/keyword/loaded/root_count/total_count/total_bytes`。
- `library` 辅助：`folder_flat_ids`、`find_folder`、`is_under`。

## 内部关键流程
1. `main.rs` 构造 Paths → `new_app_state(cx, paths)` → 内部 `db::open` + `LibraryData::reload` 加载已有索引 → `AppRoot::new` → `Root::new`。
2. 相册库「选择目录」→ `rfd::FileDialog::pick_folder` → `app.update(...start_scan(root))` → 后台线程打开独立 DB 连接执行 `scanner::scan_root`，进度经 `mpsc::channel` 回传。
3. `cx.spawn` 异步任务每 150ms 用 `gpui::background_executor().timer()` 轮询 channel：`Started/Scanning` 更新 `scan_status`，`Finished/Failed` 重新 `LibraryData::reload` 并 `cx.notify()`。
4. `page_library` 从 `data.visible_photos()` 渲染网格（按目录前缀 + 关键词过滤）；`render_folder_tree` 递归渲染 `data.folders`（Folder 类型，id 用稳定 hash 转 usize 作 gpui ElementId），点击切换 `active_folder_id` 并重算过滤。
5. 照片点击：Ctrl/Command 多选切 `selected`，否则 `open_photo_dialog` 从真实 Photo 显示文件名/日期/尺寸/大小/设备/地点/标签。
6. 首页 `page_home` 从 `data.total_count/total_bytes/root_count` 显示真实统计；无索引时展示引导空态。

## 依赖关系
- 依赖的外部模块：`core-model`（Photo/Folder）、`core-scanner`（db/scanner）、`core-config`（Paths）、`rfd`（目录对话框）、gpui/gpui-component、icon-assets。
- 被谁依赖：`main.rs` 组合根。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` → `render_page`。
- 改扫描交互：调整 `start_scan` 的后台线程与 channel 轮询逻辑。
- 缩略图真实显示：`LibraryData::thumbnail_for` 已返回缓存路径，UI 当前仍用色块占位；接入 gpui `img()` 加载该文件即可。
- 整理工具生效：为 `page_organize` 卡片补 `on_click`（M2）。

## 已知风险 / 易错点
- gpui 0.2.2 `cx.spawn` 异步闭包签名是 `FnOnce(WeakEntity<T>, &mut AsyncApp)`；异步上下文用全局 `gpui::background_executor()`，无 `cx.background_executor()`。
- gpui ElementId 不接受 `String`，目录树用 `stable_id()` 转 usize；不同目录 hash 冲突概率极低但非零。
- `state` 从 `self.app.read(cx)` 借用期间不能再可变借用 cx，故 page_* 函数开头即把数据提取为 owned 值。
- `open_dialog` 闭包是 `Fn`（非 FnOnce），捕获的 String 必须 `.clone()` 后再传入内部函数。
- rfd 目录对话框用同步 `pick_folder()`（会短暂阻塞 UI），换异步需处理 `AsyncApp` 生命周期。
- 首页外的 `page_organize` 工具卡片 badge（12组/43张等）仍为硬编码，属 M2 范围。

## 变更记录
- 2026-11-18 15:10：M1 数据流改为真实索引；新增 `ui/library.rs` 适配层，接入目录选择/后台扫描/目录过滤/真实统计；整模块重写。
- 2026-11-16 17:35：M0 从 main.rs 迁入 UI 代码。
