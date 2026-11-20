# 模块：ui-desktop

> 生成时间：2026-11-20 17:03 +0800
> 覆盖范围：src/main.rs, src/ui/analyze.rs, src/ui/app.rs, src/ui/assets.rs, src/ui/editor.rs, src/ui/library.rs, src/ui/mod.rs, src/ui/ops.rs（按路径排序）
> 覆盖 hash：25e29084480f（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面 UI 层与组合根。M1 起相册库/首页读真实 SQLite 索引（目录选择/后台扫描/目录过滤/多选/照片详情）；M2 智能整理页 4 个分析工具接入分析与后台调度；M3 新增批量删除到回收站、批量重命名（预设模板+预览+一次撤销）、回收站页（恢复/清空/30 天清理/撤销），删除绝不永久丢失原文件；M4 新增智能归档页（4 种规则 / 移动·复制 / 完整目录树预览 / 执行 / 撤销），并把整理页「智能重命名」「智能归档」卡片与侧栏「智能整理」徽标改为真实可用。缩略图真图与详情原图预览在 16:35 补充修复中提前完成。M5 完成体验闭环：首页统计/整理进度/最近操作全部接真实数据并补加载/空/错误态，新增设置与隐私中心（本机路径/保留期/数据概况/隐私说明）、窗口尺寸自适应与最小尺寸、图标注册补全、清理 M0 模拟数据与重写 readme。M6 把相册库网格改为 gpui `uniform_list` 虚拟滚动（每行 4 张、等高，只构造视口内的行）。2026-11-19 全量审查后修复：多选按照片 id、后台 channel 每轮排空、全局错误横幅、搜索框/重命名前缀输入框接 `InputState`、首页「选择目录」真正弹目录框、按钮禁用态、目录树子节点路径、弹窗按钮随文件存在性禁用。2026-11-19 11:33 修复相册库网格整体不可见：gpui `div()` 默认 `Display::Block`（非 flex），`content` 未显式开 flex 导致页面高度链断裂、`uniform_list` 拿不到确定高度渲染为 0，`grid-info` 的三个块级子元素竖排塌缩；修复为 `content` 显式 `.flex().flex_col()`、`page-library` 加 `flex_1().min_h_0()`、列表容器 `flex_1().min_h_0()` + `uniform_list` 本体 `.size_full()`、`grid-info` 加 `.flex().flex_row().items_baseline()`。同日 11:40 二次修正：照片被压成竖线是 `photo-grid-scroll` 仍为默认 Block，列表百分比宽度回退成首行 min-content；改为 `.flex().flex_col()` 让 `uniform_list` 由 cross-axis stretch 拿宽度（列表本体 `flex_1().min_h_0()`、每行 `.w_full()`、tile 加 `.min_w(px(120.))` 兜底）。2026-11-20 16:30 三次修正（真正根因）：tile 用了 `Button`，其内部独立 flex/padding 布局让 `img().size_full()` 塌陷成竖线（整理页用普通 div 所以正常）；改为与 `render_photo_list` 一致的 `div`，缩略图放进 `div().h(GRID_TILE_HEIGHT).w_full().overflow_hidden()` 容器再 `photo_thumb()`。2026-11-20 16:35 放弃虚拟滚动：三次修复仍未显示真图，回退为与整理页 `render_photo_list` 完全相同的普通 `div` + `flex_row().flex_wrap()` + 固定宽 `w(px(180))` tile，外层 `overflow_y_scroll`；移除 `uniform_list` 与 `Range` 依赖，`GRID_COLUMNS`/`GRID_ROW_HEIGHT` 换为 `GRID_TILE_WIDTH`。正确性优先，5000 张性能权衡留待后续。2026-11-20 17:03 新增无边框窗口与自绘红黄绿标题栏按钮（关闭/最小化/最大化还原、拖拽、双击最大化），去掉外层米色边框；点击任意照片改为打开独立浮窗展示原图并接入 `core-edit` 的 imageflow 编辑能力（旋转/翻转/滤镜/亮度对比度饱和度/等比缩放/另存为 PNG），信息弹窗移入浮窗的「查看图像信息」按钮。

## 对外接口
- `pub struct AppState`：字段 `page/paths/data/active_folder_id/folder_expanded/selected/scan_status/is_scanning/progress_rx/analyze_data/analyze_rx/is_analyzing/analyze_status/active_tool/ops_data/ops_status/rename_pattern/rename_prefix/last_rename_op/last_trash_op`（私有）；M4 追加 `archive_dest: PathBuf/archive_rule: ArchiveRule/archive_mode: ArchiveMode/archive_scope_all: bool/archive_plan: Option<ArchivePlan>/last_archive_op: Option<i64>`；M5 追加 `load_error: Option<String>`（启动/刷新索引失败）、`last_error: Option<String>`（最近后台任务失败）、`retention_days: i64`。2026-11-19：`selected` 由 `HashSet<usize>`（可见下标）改为 `HashSet<i64>`（照片 id）；新增 `pending_analysis: bool`（分析忙时排队，避免自动分析被静默丢弃）；`reload_all()` 失败时写 `last_error` 并保留 `active_folder`/`keyword`/`active_folder_id`，不再 `unwrap_or_default()` 清空快照，同时清空 `selected`。
- `enum Page`（私有）：`Home/Organize/Library/Rename/Archive/Trash/Settings`（M4 新增 `Archive`，M5 新增 `Settings`）；`pub enum OrganizeTool`：`Similar/Blurry/Screenshot/Large`（整理页当前工具）。
- `pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState>`：创建初始状态并从索引加载 LibraryData + AnalyzeData + OpsData；M4 起还从 `settings` 读归档配置（`archive.dest/rule/mode`）并生成首份 `archive_plan`；M5 起读 `trash.retention_days`，db 打开失败时记录 `load_error`。
- `AppState::visible_count() -> usize`、`start_scan(root, cx)`、`start_analysis(cx)`：分别启动后台扫描与分析；`start_scan`/`start_analysis` 的轮询每轮**排空 channel**，扫描完成调用 `reload_all()` 且仅在 added/updated/removed > 0 时触发分析，分析完成也 `reload_all()`。
- `AppState` 私有方法（M3）：`reload_all()`（刷新三份快照）、`selected_photo_ids() -> Vec<i64>`（直接返回照片 id 集合，不再由可见下标映射）、`batch_trash()`、`batch_rename()`、`undo_rename()`、`undo_trash()`。
- `AppState` 私有方法（M4）：`archive_ids() -> Vec<i64>`、`persist_archive_settings()`、`refresh_archive_plan()`（调 `ops_archive::plan`，未选目标时默认「扫描根目录的同级 / 归档」）、`set_archive_dest()`（替换原 `choose_archive_dest`；`rfd::FileDialog` 在 `app.update` 外打开，并校验目标不在扫描根内）、`execute_archive()`、`undo_archive()`。侧栏进入 `Page::Archive` 时自动 `refresh_archive_plan()`。
- `pub struct AppRoot` + `AppRoot::new(app: Entity<AppState>)` + `impl Render`（`render(&mut self, window: &mut Window, cx: ...)`）；2026-11-19 新增 `search_input: Option<Entity<InputState>>`、`prefix_input: Option<Entity<InputState>>`、`_subscriptions`，由 `ensure_inputs(window, cx)` 创建并订阅 `InputEvent::Change`，分别同步 `data.keyword` 与 `rename_prefix`。
- `pub struct Assets` + `impl AssetSource`（src/ui/assets.rs，编译期嵌入图标）。
- `LibraryData`（src/ui/library.rs）：`reload(conn)`、`visible_photos()`、`thumbnail_for(&Photo)`；`build_folder_tree` 用完整相对父目录 `root_path.join(parent_rel)` 作为子节点 path，根目录直接摆放的照片不再生成伪「（根目录）」子节点。
- `AnalyzeData`（src/ui/analyze.rs，M2）：`reload(conn) -> Result<Self>`、`badges() -> [String;4]`、`is_ready() -> bool`；字段 `groups/blurry/screenshots/large/analyzed_count/loaded`。
- `OpsData`（src/ui/ops.rs，M3）：`reload(conn) -> Result<Self>`、`size_label() -> String`；字段 `trash: Vec<TrashItem>/trash_count/trash_bytes/recent_ops: Vec<OpLog>/loaded`。
- **图片渲染 helper（src/ui/app.rs，模块内私有 fn）**：`is_decodable_image(&Path) -> bool`；`thumbnail_source(&Photo) -> Option<PathBuf>`（缩略图缓存优先，`photo.path` 不存在时返回 `None`，避免幽灵缩略图）；`preview_source(&Photo) -> Option<PathBuf>`；`photo_placeholder(hue, location, name) -> AnyElement`；`photo_thumb(&Photo) -> AnyElement`（`img()` + `ObjectFit::Cover` + fallback/loading 降级）。
- `pub(crate) fn open_photo_dialog(photo: Photo, window: &mut Window, cx: &mut App)`：接受 owned Photo，弹窗左侧原图大图（`ObjectFit::Contain`），右侧基本信息/分析结果/索引字段；「打开原图」「所在文件夹」按 `photo.path.exists()` 禁用并改文案。由浮窗「查看图像信息」按钮调用。
- **窗口装饰与标题栏（src/ui/mod.rs，2026-11-20）**：`pub fn window_titlebar(title: impl Into<SharedString>, badge: impl Into<SharedString>) -> AnyElement` 生成自绘标题栏；模块内私有 `traffic_button(id, color, area, action)` / `window_controls()`。红=关闭（`Window::remove_window`）、黄=最小化（`minimize_window`）、绿=最大化/还原（`zoom_window`）；三按钮均设 `WindowControlArea::{Close,Min,Max}` 接 Windows 命中测试，`on_click` 为其他平台兜底；标题区设 `WindowControlArea::Drag` 并双击 `zoom_window`（macOS 走 `titlebar_double_click`）。macOS 隐藏自绘点、保留系统原生红绿灯位置。
- **原图编辑浮窗（src/ui/editor.rs，2026-11-20）**：`pub struct PhotoEditor` 实现 `Render`；`pub fn open_photo_editor(photo: Photo, paths: Paths, cx: &mut App)` 通过 `cx.open_window` 创建 `WindowKind::Floating` 窗口并包 `gpui_component::Root::new`。状态含 `photo/source/paths/label/preview/options/status/error/busy/generation`；`mutate()` 改参数并 `schedule_preview()`，`generation` 防后台旧任务覆盖新预览；`save_as()` 用 `max_dimension: 0` 调 `core::edit::apply` 后 `std::fs::copy` 到 rfd 另存为路径。预览任务由 `cx.background_executor().spawn` 执行，`cx.spawn` 回主线程提交；浮窗用 `window_titlebar` 同款标题栏。
- **M4 归档页（src/ui/app.rs，模块内私有）**：`AppRoot::page_archive(&self, state: &AppState) -> impl IntoElement + 'static`——① 规则卡片遍历 `ArchiveRule::ALL`；② 模式按钮（Move/Copy）；③ 范围按钮；④ 目标目录按钮（`set_archive_dest`）；左侧完整目录树预览，右侧统计面板；底部「刷新预览」「撤销上次归档」「执行归档（N 张）」。
- **M4 归档渲染 helper（模块内私有 fn）**：`flatten_archive_node(...)`、`archive_stat_row(...)`。
- **M5 设置与隐私页（src/ui/app.rs，模块内私有）**：`AppRoot::page_settings(&self, state: &AppState) -> impl IntoElement + 'static`——5 张卡片（本机数据 / 整理偏好 / 数据概况 / 隐私说明 / 关于）；错误横幅改由全局 `global-error-banner` 统一展示。
- **相册库网格常量（src/ui/app.rs，2026-11-20 16:35 起）**：`const GRID_TILE_WIDTH: f32 = 180.`、`const GRID_TILE_HEIGHT: f32 = 150.`；`uniform_list`/`Range`/`GRID_COLUMNS`/`GRID_ROW_HEIGHT` 已移除，网格改用普通 `div().flex().flex_row().flex_wrap()` + 固定宽 tile。
- **2026-11-19 新增交互**：搜索框 `Input::new(&st)`（相册库）与重命名前缀输入框（重命名页）；批量按钮接 `gpui_component::Disableable` 的 `.disabled(...)`（重命名执行/撤销、归档执行/撤销、回收站撤销、详情弹窗按钮）；全局错误横幅 `global-error-banner`；`undo_trash` 返回 `Ok(0)` 时提示「没有可恢复的项目（可能已被清空或过期清理）」；`empty_trash`/`purge_expired` 成功后清空 `last_trash_op`；`age_days` 用 `.max(0)` 防时钟回拨负数。
- 其他辅助：`format_millis(i64) -> String`、`human_size(i64) -> String`、`truncate(&str, usize) -> String`、`subtitle_for_home(&AppState) -> String`、`open_with_default(&Path)`、`reveal_in_explorer(&Path)`、`info_row`、`info_block`、`section_title`、`info_tag_section`、`dialog_btn`。
- `library` 辅助：`folder_flat_ids`、`find_folder`、`is_under`。
- **组合根（src/main.rs）**：`main()` → `Paths::platform_default()`（失败回退 `dirs::home_dir()` 下的 `.akashic`，不再优先系统临时目录）→ `ensure_dirs()` 失败直接打印并 `return`（不再启动出静默失败 UI）→ `cx.open_window(...)` 的 `Err` 用 `eprintln!` 打印而非被 `detach()` 吞掉；`WindowOptions.titlebar = Some(TitlebarOptions { title: "Akashic", appears_transparent: true, traffic_light_position: macOS 时 (14,16) })` 隐藏系统标题栏，改由 `window_titlebar` 自绘。

## 内部关键流程
1. `main.rs` 构造 Paths → `new_app_state(cx, paths)` → 内部 `db::open` + `LibraryData::reload` 加载已有索引 → `AppRoot::new` → `Root::new`。
2. 相册库 / 首页「选择目录」→ `rfd::FileDialog::pick_folder` → `app.update(...start_scan(root))` → 后台线程打开独立 DB 连接执行 `scanner::scan_root`，进度经 `mpsc::channel` 回传。
3. `cx.spawn` 异步任务每 150ms 用 `gpui::background_executor().timer()` 轮询 channel：**每轮用 `try_iter()` 排空全部消息**（此前每轮只收一条，进度消息积压会让 UI 永远停在「扫描中/分析中」）；`Started/Scanning` 更新 `scan_status`，`Finished/Failed` 重新 `reload_all()` 并 `cx.notify()`；entity 释放时 break。
4. `page_library` 从 `data.visible_photos()` 克隆为 owned `Vec<Photo>`，按目录前缀 + 关键词过滤（`keyword` 由搜索框 `InputState` 写入）；2026-11-20 16:35 起用与整理页一致的普通 `div` + `flex_row().flex_wrap()` 网格（固定 `w(180)` tile，`overflow_y_scroll`），不再用 `uniform_list`；`render_folder_tree` 递归渲染 `data.folders`。外层高度链仍需 `content`（`.flex().flex_col()`）→ `page-library`（`flex_1().min_h_0()`）→ `grid-panel`（`flex_1()`）逐层确定，否则列表高度为 0。
5. 照片点击：`ev.modifiers().control || ev.modifiers().platform` 时按 **`photo.id`** 插入 / 移除 `selected`（不再用可见下标，避免筛选或排序后删错照片）；普通点击调 `open_photo_editor(photo, paths, cx)` 打开独立浮窗展示原图并编辑，浮窗内「查看图像信息」才调 `open_photo_dialog`。整理结果、相似组、回收站缩略图点击同样走 `open_photo_editor`。
6. 首页 `page_home`（M5 重写）：4 张真实统计卡来自 `data.total_count/total_bytes/root_count/visible_photos().len()`；状态横幅按「错误态 > 加载态 > 隐藏」渲染；「整理进度」显示 `AnalyzeData` 真实计数；「最近整理记录」取 `OpsData.recent_ops` 前 6 条，用 `OpKind::label()` + `item_count` + `format_millis` + `truncate`，空列表显示引导空态。
7. **M2 分析链路**：扫描 `Finished` 后自动调用 `start_analysis`（也可整理页点「重新分析」手动触发）；分析忙时用 `pending_analysis` 排队而非静默丢弃 → 后台线程 `core::analyze::analyze_library` 写库 → `AnalyzeProgress` 经 channel 回传 → 第二个 `cx.spawn` 轮询（200ms，每轮排空）刷新 `analyze_status` 与 `AnalyzeData::reload`；完成后 `reload_all()`。
8. 整理页 `page_organize(state)`：6 张工具卡片徽标改真实值；前 4 张 `on_click` 设 `active_tool` 进入结果列表；M4 起后 2 张跳 `Page::Rename` / `Page::Archive`；`render_tool_result` 按工具分发；结果项与相似组卡片用 `photo_thumb`，`on_click` 打开详情弹窗。侧栏「智能整理」徽标为真实合计。
9. **M3 批量删除**：相册库工具栏「删除到回收站」→ `AppState::batch_trash` → `selected_photo_ids()` → `core::ops::trash::trash_photos` → `reload_all()` 刷新并清空选中；撤销走 `undo_trash()`。
10. **M3 批量重命名**：工具栏「重命名」跳 `Page::Rename` → `page_rename` 展示 5 个预设模板卡片 + **前缀输入框（`InputState`）** + 用当前 pattern/prefix 经 `ops_rename::preview` 计算前 5 条实时预览 → 「执行重命名」调 `batch_rename()` → 「撤销上次重命名」调 `undo_rename()`。执行/撤销按钮在无操作 / 忙碌时 `.disabled(...)`。
11. **M3 回收站页**：`page_trash` 从 `OpsData.trash` 渲染列表 → 恢复调 `ops_trash::restore_items`；底部「撤销上次删除」「清理 N 天前」「清空回收站」分别调 `undo_trash`/`purge_expired(conn, s.retention_days)`/`empty_trash`；撤销按钮在无可撤销操作时禁用。行首缩略图用 `photo_thumb`，点击打开详情并把 `photo.path` 重定向到 `TrashItem.trash_path`。
12. **图片渲染（16:35 新增）**：所有照片展示位统一走 `photo_thumb(&Photo)`；`gpui::img(PathBuf)` 走 `Resource::Path` → `fs::read` + `image` 解码，**不经过只认图标的 `Assets`**；`with_fallback` 与 `with_loading` 均降级到 `photo_placeholder`。扩展名不在 `Img::extensions()` 内由 `is_decodable_image` 拦下。
13. **M4 归档链路**：`new_app_state` 从 `settings` 读 `archive.dest/rule/mode` → `refresh_archive_plan()` 生成只读计划 → `page_archive` 展平树渲染预览 + 统计。切换规则/模式/范围或 `rfd` 选目标目录时先更新 AppState + `persist_archive_settings()` 再重算预览。「执行归档」先再次 `refresh_archive_plan()` → `ops_archive::execute` → `reload_all()` + 重算预览；「撤销上次归档」调 `ops_archive::undo`。
14. **M5 首页数据链路**：`new_app_state` 加载三份快照 → `page_home` 一次性提取 owned 值 → 统计卡 / 进度区 / 最近操作列表全部读索引与 `op_log`，无任何模拟数字。
15. **M5 保留期设置链路**：设置页按钮 → `db::open` + `ops_trash::set_retention_days` → 更新 `AppState.retention_days`；回收站页 purge 按钮标签、`purge_expired(&conn, s.retention_days)`、结果文案均跟随该值，不再硬编码 30。
16. **M5 错误/加载态（2026-11-19 改为全局）**：`start_scan`/`start_analysis` 的 `Finished` 清 `last_error`、`Failed` 写 `last_error`；`reload_all` 成功清 `load_error`、失败写入；**全局错误横幅 `global-error-banner` 在所有页面统一展示**，首页 banner 只保留进度态，设置页不再单独渲染错误横幅。
17. **M5 窗口与图标资源**：`main.rs` 的 `WindowOptions.window_min_size = Some(Size::new(px(960.), px(640.)))`，1200×780 仅作初始 bounds；`AppRoot::render` 的客户区 `.size_full()` 直接铺满（此前 `windowcard` 圆角 + 米色底色形成额外边框，已移除）；`Assets::load` 登记 `assets/icons/` 下全部 47 个 SVG。
18. **2026-11-20 原图编辑浮窗链路**：四处照片点击（整理结果/相似组/相册库/回收站）各自捕获 `paths.clone()` → `open_photo_editor(photo, paths, cx)` → `cx.open_window(WindowOptions { kind: Floating, titlebar.appears_transparent: true, ... })` → `PhotoEditor::new` 用原图作初始 `preview`。每次编辑参数变更 `generation += 1` → 后台 `core::edit::apply` 生成 `paths.data_dir/editor/edit-<hash>.png` → 回主线程仅在代次一致时提交 `preview` 并 `cx.notify()`；另存为用 `max_dimension: 0` 生成原分辨率 PNG，`rfd::FileDialog::save_file()` 在 `app.update` 之外调用。
19. **2026-11-19 输入框链路**：`AppRoot::render`（`&mut Window`）调用 `ensure_inputs(window, cx)` 惰性创建 `search_input`/`prefix_input` 两个 `InputState` 实体并订阅 `InputEvent::Change`；搜索框变更写 `st.data.keyword`、前缀框变更写 `st.rename_prefix`，随后 `cx.notify()` 触发重绘。`_subscriptions` 持有订阅以保活。

## 依赖关系
- 依赖的外部模块：`core-model`（Photo/Folder/PhotoGroup/OpLog/TrashItem/OpKind）、`core-scanner`（db/scanner/settings）、`core-analyze`（AnalyzeProgress/analyze_library）、`core-ops`（trash/rename/archive 批量操作）、`core-edit`（imageflow 编辑流水线，浮窗调用）、`core-config`（Paths）、`rfd`（目录选择与另存为对话框）、gpui/gpui-component（`Input`/`InputState`/`InputEvent`/`Disableable`）、icon-assets。
- 被谁依赖：`main.rs` 组合根（本模块覆盖范围已含 `src/main.rs`）。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` → `render_page`（M5 新增 `Page::Settings` 与 `page_settings`）。
- 新增状态态：错误/加载信息统一写 `AppState.last_error/load_error`，页面读同一份状态；错误展示已收归全局横幅，勿各页自定义错误横幅。
- 新增设置项：`page_settings` 加卡片 → core 提供 get/set（复用 `db::get_setting/set_setting`）→ 在 `tests/settings_test.rs` 补用例。
- 新增文本输入：复用 `ensure_inputs` 的 `InputState` 模式（`window`/`cx` 创建 + `_subscriptions` 保活 + `InputEvent::Change` 同步到 AppState），勿在渲染闭包内临时创建。
- 新增图标：SVG 放入 `assets/icons/` 后必须在 `Assets::load` 登记，缺登记会让 gpui-component 图标空白。
- 虚拟网格：列数/行高常量在 app.rs 顶部；改列数需同步 `row_count` 与 `idx` 计算，行必须等高，否则 uniform_list 测量会错位。
- 改扫描交互：调整 `start_scan` 的后台线程与 channel 轮询逻辑；**每轮必须排空 channel**。
- 缩略图显示：已统一在 `photo_thumb`（app.rs）；换图源/尺寸改 `thumbnail_source`/`preview_source`；新增展示位直接复用 `photo_thumb(&Photo)`。
- 分析结果交互：在 `render_photo_list`/`render_similar_groups` 增加「保留最佳」/删除动作（删除可复用 `AppState::batch_trash` 的选中 id 路径）。
- 归档规则/范围：加规则改 `core-ops::archive::ArchiveRule`（UI 遍历 `ALL` 自动跟随）；改目标目录走 `set_archive_dest`（rfd 弹窗须在 `app.update` 外打开）。
- 批量按钮：无可执行操作 / 忙碌时用 `Disableable::disabled(...)` 禁用，避免空操作或重入。
- 批量操作进度：当前删除/重命名同步执行（适合个人库规模）；量大时可参照扫描/分析改后台线程 + channel。

## 已知风险 / 易错点
- gpui 0.2.2 `cx.spawn` 异步闭包签名是 `FnOnce(WeakEntity<T>, &mut AsyncApp)`；异步上下文用全局 `gpui::background_executor()`，无 `cx.background_executor()`。
- **channel 积压**：后台进度消息若每轮只收一条，5000 张会永远显示「扫描中/分析中」；轮询必须 `try_iter()` 排空，后台侧也要节流（`PROGRESS_EVERY=32`）。
- **多选口径**：`selected` 必须是照片 id（`HashSet<i64>`），不能用可见下标；筛选/排序/虚拟滚动后下标会变，用下标会删错照片。
- gpui ElementId 不接受 `String`，目录树用 `stable_id()` 转 usize；不同目录 hash 冲突概率极低但非零。
- `state` 从 `self.app.read(cx)` 借用期间不能再可变借用 cx，故 page_* 函数开头即把数据提取为 owned 值。
- `open_dialog` 闭包是 `Fn`（非 FnOnce），捕获的 String 必须 `.clone()` 后再传入内部函数。
- rfd 目录对话框用同步 `pick_folder()`（会短暂阻塞 UI），必须在 `app.update` 外调用，否则重入/借用冲突；换异步需处理 `AsyncApp` 生命周期。
- `InputState` 需要在 `Render::render(&mut Window)` 中创建并持有 `_subscriptions`，否则订阅被释放后输入框不更新状态。
- `Disableable` trait 必须显式 `use gpui_component::Disableable;`，否则 `Button::disabled` 编译报 E0599。
- gpui 0.2.2 中 `AnyElement` 非 struct，函数返回统一用 `impl IntoElement + 'static`，分支内 `.into_any_element()` 后保持同型。
- `img(PathBuf)` 走 `Resource::Path` 的 `fs::read`，路径必须是本地绝对路径；`Assets`（src/ui/assets.rs）只服务 `Resource::Embedded` 图标，不参与照片加载。
- 图片是异步 asset cache：`with_fallback` 只在解码失败时显示、`with_loading` 只在超时未完成显示，两者都要给，否则加载中白屏。
- `open_dialog` 渲染闭包是 `Fn`（每次重绘调用），大图元素必须每次重建；详情按钮须按 `photo.path.exists()` 禁用，回收站项若不重定向 `photo.path` 到 `trash_path` 会指向失效路径。
- 归档目标目录不要落在扫描根目录内，否则下次扫描会把归档副本当新照片重复索引；`set_archive_dest` 已校验，UI 默认目标是「扫描根目录的同级 / 归档」。
- 归档执行前必须重算 `archive_plan`，否则用户改了规则/范围后落盘与预览不一致；`execute_archive()` 已内置刷新。
- 不支持格式（HEIC/RAW 等）由 `is_decodable_image` 拦下：此时网格仍可用缩略图缓存，详情弹窗经 `preview_source` 回退到缩略图。
- 错误态只显示最近一条；`last_error` 在下次后台任务成功时清除，历史错误不保留。
- 相册库列数固定为 `w(180)` tile + `flex_wrap`（gpui 0.2.2 无响应式断点），窄窗口靠 wrap 与滚动；`window_min_size` 保证侧栏与内容区不挤压。
- 自绘红黄绿按钮在 Windows 同时依赖 `WindowControlArea` 命中测试与 `on_click` 兜底；新增控制按钮时必须同时设置两者，否则可能出现「看着像按钮但点了没反应」。macOS 自绘按钮隐藏，依赖系统原生红绿灯。
- 原图浮窗的 `PhotoEditor` 在 `Render` 里持有后台任务代次 `generation`：任何参数改动都要走 `mutate()`/`schedule_preview()`，不要直接写 `preview`，否则旧任务会覆盖新结果。
- rfd 另存为对话框必须在 `app.update` 之外调用；编辑任务不要在 UI 线程同步执行（`core::edit::apply` 阻塞）。
- 弹窗叠加、rfd 重入、GUI 自动化冒烟测试本期未验证。

## 变更记录
- 2026-11-20 17:03：`main.rs` 隐藏系统标题栏并去掉外层米色边框；新增 `src/ui/mod.rs::window_titlebar` 自绘红黄绿按钮（关闭/最小化/最大化还原 + 拖拽 + 双击）；新增 `src/ui/editor.rs` 独立原图编辑浮窗并接入 `core-edit`；四处照片点击改开浮窗，信息弹窗改由浮窗按钮触发；覆盖范围纳入 `src/ui/editor.rs`，刷新 hash。
- 2026-11-19 10:38：全量 bug 审查修复——`selected` 改为 `HashSet<i64>`、后台 channel 每轮排空 + `pending_analysis` 排队、全局错误横幅、搜索框/前缀框接 `InputState`、首页「选择目录」接 `rfd`、多处补 `cx.notify()`、按钮 `Disableable` 禁用态、目录树子节点路径修正、`thumbnail_source` 校验文件存在、弹窗按钮按存在性禁用、`age_days().max(0)`、`set_archive_dest` 在 `app.update` 外弹窗并校验目标；`main.rs` 启动失败不再静默继续；刷新 hash 并把 `src/main.rs` 纳入覆盖范围（与 INDEX 目录职责表一致）。
- 2026-11-19：M6 相册库网格改 `uniform_list` 虚拟滚动（每行 4 张 / 等高 162px / 只渲染视口内行），新增 `GRID_COLUMNS`/`GRID_TILE_HEIGHT`/`GRID_ROW_HEIGHT` 常量与 `std::ops::Range` 导入；多选与详情点击保持原全局索引口径。刷新 hash。
- 2026-11-18 17:40：M5 首页改真实最近操作/整理进度/加载·空·错误态，新增 `Page::Settings` 与 `page_settings`，AppState 增加 3 个状态字段并维护后台失败信息，回收站清理按钮跟随设置值；`windowcard` 改 `.size_full()` 配合 main.rs `window_min_size`；`Assets::load` 登记全部 47 个 SVG；新增 `human_size`/`truncate` helper。刷新 hash。
- 2026-11-18 16:58：M4 新增 `Page::Archive` + 侧栏导航 + `page_archive`，AppState 增加归档配置与计划快照并持久化到 settings；整理页后 2 张卡片改为可跳转、侧栏「智能整理」徽标改真实合计；新增 `flatten_archive_node`/`archive_stat_row` helper。刷新 hash。
- 2026-11-18 16:35：修复「能看到数量但看不到缩略图、点击无原图/信息」——新增图片渲染 helper 组，所有展示位接入 gpui `img(PathBuf)` 真实缩略图；`open_photo_dialog` 改接受 owned Photo，弹窗升级为原图大图 + 分区信息 + 打开原图/所在文件夹按钮。刷新 hash。
- 2026-11-18 16:05：M3 相册库工具栏接批量删除/重命名，新增重命名页与回收站页，新增 `ui/ops.rs` 适配层、AppState 批量操作方法。刷新 hash。
- 2026-11-18 15:40：M2 整理页工具卡片接真实分析结果、新增后台分析调度与结果列表，新增 `ui/analyze.rs` 适配层。刷新 hash。
- 2026-11-18 15:10：M1 数据流改为真实索引；新增 `ui/library.rs` 适配层。整模块重写。
- 2026-11-16 17:35：M0 从 main.rs 迁入 UI 代码。
