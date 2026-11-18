# 模块：ui-desktop

> 生成时间：2026-11-18 15:40 +0800
> 覆盖范围：src/ui/analyze.rs, src/ui/app.rs, src/ui/assets.rs, src/ui/library.rs, src/ui/mod.rs（按路径排序）
> 覆盖 hash：379e4b66b661（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面 UI 层。M1 起相册库/首页读真实 SQLite 索引（目录选择/后台扫描/目录过滤/多选/照片详情）；M2 智能整理页 4 个分析工具（相似/模糊/截图/大文件）接入分析结果与后台分析调度，可点击进入结果列表；重命名/回收站仍为占位。

## 对外接口
- `pub struct AppState`：字段 `page/paths/data/analyze_data/active_folder_id/folder_expanded/selected/scan_status/is_scanning/progress_rx/analyze_rx/is_analyzing/analyze_status/active_tool`（私有）。
- `pub enum OrganizeTool`：`Similar/Blurry/Screenshot/Large`（整理页当前工具）。
- `pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState>`：创建初始状态并从索引加载 LibraryData + AnalyzeData。
- `AppState::visible_count() -> usize`、`start_scan(root, cx)`、`start_analysis(cx)`：分别启动后台扫描与分析。
- `pub struct AppRoot` + `AppRoot::new(app: Entity<AppState>)` + `impl Render`。
- `pub struct Assets` + `impl AssetSource`（src/ui/assets.rs，编译期嵌入图标）。
- `LibraryData`（src/ui/library.rs）：`reload(conn)`、`visible_photos()`、`thumbnail_for(&Photo)`；字段见源码。
- `AnalyzeData`（src/ui/analyze.rs，M2）：`reload(conn) -> Result<Self>`、`badges() -> [String;4]`、`is_ready() -> bool`；字段 `groups/blurry/screenshots/large/analyzed_count/loaded`。
- `library` 辅助：`folder_flat_ids`、`find_folder`、`is_under`。

## 内部关键流程
1. `main.rs` 构造 Paths → `new_app_state(cx, paths)` → 内部 `db::open` + `LibraryData::reload` 加载已有索引 → `AppRoot::new` → `Root::new`。
2. 相册库「选择目录」→ `rfd::FileDialog::pick_folder` → `app.update(...start_scan(root))` → 后台线程打开独立 DB 连接执行 `scanner::scan_root`，进度经 `mpsc::channel` 回传。
3. `cx.spawn` 异步任务每 150ms 用 `gpui::background_executor().timer()` 轮询 channel：`Started/Scanning` 更新 `scan_status`，`Finished/Failed` 重新 `LibraryData::reload` 并 `cx.notify()`。
4. `page_library` 从 `data.visible_photos()` 渲染网格（按目录前缀 + 关键词过滤）；`render_folder_tree` 递归渲染 `data.folders`（Folder 类型，id 用稳定 hash 转 usize 作 gpui ElementId），点击切换 `active_folder_id` 并重算过滤。
5. 照片点击：Ctrl/Command 多选切 `selected`，否则 `open_photo_dialog` 从真实 Photo 显示文件名/日期/尺寸/大小/设备/地点/标签。
6. 首页 `page_home` 从 `data.total_count/total_bytes/root_count` 显示真实统计；无索引时展示引导空态。
7. **M2 分析链路**：扫描 `Finished` 后自动调用 `start_analysis`（也可整理页点「重新分析」手动触发）→ 后台线程 `core::analyze::analyze_library` 写库 → `AnalyzeProgress` 经 channel 回传 → 第二个 `cx.spawn` 轮询（200ms）刷新 `analyze_status` 与 `AnalyzeData::reload`。
8. 整理页 `page_organize(state)`：6 张工具卡片徽标改真实值（前 4 个来自 `AnalyzeData::badges()`）；前 4 张 `on_click` 设 `active_tool` 进入结果列表；`render_tool_result` 按工具分发到相似组列表 / 通用照片列表；结果项用色块占位（缩略图真图接入同 M5）。

## 依赖关系
- 依赖的外部模块：`core-model`（Photo/Folder/PhotoGroup）、`core-scanner`（db/scanner）、`core-analyze`（AnalyzeProgress/analyze_library）、`core-config`（Paths）、`rfd`（目录对话框）、gpui/gpui-component、icon-assets。
- 被谁依赖：`main.rs` 组合根。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` → `render_page`。
- 改扫描交互：调整 `start_scan` 的后台线程与 channel 轮询逻辑。
- 缩略图真实显示：`LibraryData::thumbnail_for` 已返回缓存路径，相册库与整理结果列表当前仍用色块占位；接入 gpui `img()` 加载该文件即可（M5）。
- 分析结果交互：在 `render_photo_list`/`render_similar_groups` 增加「保留最佳」/删除动作（删除属 M3 回收站范围）。

## 已知风险 / 易错点
- gpui 0.2.2 `cx.spawn` 异步闭包签名是 `FnOnce(WeakEntity<T>, &mut AsyncApp)`；异步上下文用全局 `gpui::background_executor()`，无 `cx.background_executor()`。
- gpui ElementId 不接受 `String`，目录树用 `stable_id()` 转 usize；不同目录 hash 冲突概率极低但非零。
- `state` 从 `self.app.read(cx)` 借用期间不能再可变借用 cx，故 page_* 函数开头即把数据提取为 owned 值。
- `open_dialog` 闭包是 `Fn`（非 FnOnce），捕获的 String 必须 `.clone()` 后再传入内部函数。
- rfd 目录对话框用同步 `pick_folder()`（会短暂阻塞 UI），换异步需处理 `AsyncApp` 生命周期。
- gpui 0.2.2 中 `AnyElement` 非 struct，函数返回统一用 `impl IntoElement + 'static`，分支内 `.into_any_element()` 后保持同型。

## 变更记录
- 2026-11-18 15:40：M2 整理页工具卡片接真实分析结果、新增后台分析调度与结果列表，新增 `ui/analyze.rs` 适配层，刷新 hash。
- 2026-11-18 15:10：M1 数据流改为真实索引；新增 `ui/library.rs` 适配层，接入目录选择/后台扫描/目录过滤/真实统计；整模块重写。
- 2026-11-16 17:35：M0 从 main.rs 迁入 UI 代码。
