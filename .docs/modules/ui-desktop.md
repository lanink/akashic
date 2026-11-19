# 模块：ui-desktop

> 生成时间：2026-11-19 08:54 +0800
> 覆盖范围：src/ui/analyze.rs, src/ui/app.rs, src/ui/assets.rs, src/ui/library.rs, src/ui/mod.rs, src/ui/ops.rs（按路径排序）
> 覆盖 hash：cce1cd37a7c9（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面 UI 层。M1 起相册库/首页读真实 SQLite 索引（目录选择/后台扫描/目录过滤/多选/照片详情）；M2 智能整理页 4 个分析工具接入分析与后台调度；M3 新增批量删除到回收站、批量重命名（预设模板+预览+一次撤销）、回收站页（恢复/清空/30 天清理/撤销），删除绝不永久丢失原文件；M4 新增智能归档页（4 种规则 / 移动·复制 / 完整目录树预览 / 执行 / 撤销），并把整理页「智能重命名」「智能归档」卡片与侧栏「智能整理」徽标改为真实可用。缩略图真图与详情原图预览在 16:35 补充修复中提前完成。M5 完成体验闭环：首页统计/整理进度/最近操作全部接真实数据并补加载/空/错误态，新增设置与隐私中心（本机路径/保留期/数据概况/隐私说明）、窗口尺寸自适应与最小尺寸、图标注册补全、清理 M0 模拟数据与重写 readme。M6 把相册库网格改为 gpui `uniform_list` 虚拟滚动（每行 4 张、等高，只构造视口内的行），补齐 5000 张场景的性能支撑。

## 对外接口
- `pub struct AppState`：字段 `page/paths/data/active_folder_id/folder_expanded/selected/scan_status/is_scanning/progress_rx/analyze_data/analyze_rx/is_analyzing/analyze_status/active_tool/ops_data/ops_status/rename_pattern/rename_prefix/last_rename_op/last_trash_op`（私有）；M4 追加 `archive_dest: PathBuf/archive_rule: ArchiveRule/archive_mode: ArchiveMode/archive_scope_all: bool/archive_plan: Option<ArchivePlan>/last_archive_op: Option<i64>`；M5 追加 `load_error: Option<String>`（启动/刷新索引失败）、`last_error: Option<String>`（最近后台任务失败）、`retention_days: i64`（设置页可改）。
- `enum Page`（私有）：`Home/Organize/Library/Rename/Archive/Trash/Settings`（M4 新增 `Archive`，M5 新增 `Settings`）；`pub enum OrganizeTool`：`Similar/Blurry/Screenshot/Large`（整理页当前工具）。
- `pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState>`：创建初始状态并从索引加载 LibraryData + AnalyzeData + OpsData；M4 起还从 `settings` 读归档配置（`archive.dest/rule/mode`）并生成首份 `archive_plan`；M5 起读 `trash.retention_days`（`ops_trash::retention_days`），db 打开失败时记录 `load_error`。
- `AppState::visible_count() -> usize`、`start_scan(root, cx)`、`start_analysis(cx)`：分别启动后台扫描与分析。
- `AppState` 私有方法（M3）：`reload_all()`（刷新三份快照）、`selected_photo_ids() -> Vec<i64>`（多选索引→照片 id）、`batch_trash()`、`batch_rename()`、`undo_rename()`、`undo_trash()`。
- `AppState` 私有方法（M4）：`archive_ids() -> Vec<i64>`（按 `archive_scope_all` 取全部可见或仅选中）、`persist_archive_settings()`（写 `settings` 表）、`refresh_archive_plan()`（调 `ops_archive::plan`，未选目标时默认「扫描根目录的同级 / 归档」）、`choose_archive_dest()`（`rfd::FileDialog::pick_folder`）、`execute_archive()`（先刷新计划再 `ops_archive::execute`，随后 `reload_all` + 重算预览）、`undo_archive()`（`ops_archive::undo`）。
- `pub struct AppRoot` + `AppRoot::new(app: Entity<AppState>)` + `impl Render`。
- `pub struct Assets` + `impl AssetSource`（src/ui/assets.rs，编译期嵌入图标）。
- `LibraryData`（src/ui/library.rs）：`reload(conn)`、`visible_photos()`、`thumbnail_for(&Photo)`；字段见源码。
- `AnalyzeData`（src/ui/analyze.rs，M2）：`reload(conn) -> Result<Self>`、`badges() -> [String;4]`、`is_ready() -> bool`；字段 `groups/blurry/screenshots/large/analyzed_count/loaded`。
- `OpsData`（src/ui/ops.rs，M3）：`reload(conn) -> Result<Self>`、`size_label() -> String`；字段 `trash: Vec<TrashItem>/trash_count/trash_bytes/recent_ops: Vec<OpLog>/loaded`。
- **图片渲染 helper（src/ui/app.rs，本次新增，均为模块内私有 fn）**：`is_decodable_image(&Path) -> bool`（扩展名是否落在 `gpui::Img::extensions()`）；`thumbnail_source(&Photo) -> Option<PathBuf>`（缩略图缓存优先，缺失回退原图）；`preview_source(&Photo) -> Option<PathBuf>`（原图优先，不可解码回退缩略图）；`photo_placeholder(hue: f32, location: &str, name: &str) -> AnyElement`（色块降级）；`photo_thumb(&Photo) -> AnyElement`（`img()` + `ObjectFit::Cover` + fallback/loading 降级）。
- `fn open_photo_dialog(photo: Photo, window: &mut Window, cx: &mut App)`：改为接受 owned Photo（不再依赖 `AppState`/visible 索引），相册库/整理结果/相似组/回收站均可调用；弹窗左侧原图大图（`ObjectFit::Contain`），右侧分区显示基本信息/分析结果/索引字段，「打开原图」「所在文件夹」调系统命令。
- **M4 归档页（src/ui/app.rs，模块内私有）**：`AppRoot::page_archive(&self, state: &AppState) -> impl IntoElement + 'static`——① 规则卡片遍历 `ArchiveRule::ALL`（点击改规则并持久化+重算预览）；② 模式按钮（Move/Copy，用 `Button::when` 切 primary/outline）；③ 范围按钮（全部可见 / 仅选中）；④ 目标目录按钮（`choose_archive_dest`）；左侧完整目录树预览（展平后按深度缩进，显示每层张数/占用/样例文件名），右侧统计面板（待归档/占用/冲突改名/规则/模式/目标/复制模式说明）；底部「刷新预览」「撤销上次归档」「执行归档（N 张）」。
- **M4 归档渲染 helper（模块内私有 fn）**：`flatten_archive_node(node: &ArchiveTreeNode, depth: usize, out: &mut Vec<(usize, ArchiveTreeNode)>)`（递归展平目录树）、`archive_stat_row(label: &'static str, value: String) -> AnyElement`（统计行）。
- **M5 设置与隐私页（src/ui/app.rs，模块内私有）**：`AppRoot::page_settings(&self, state: &AppState) -> impl IntoElement + 'static`——局部闭包 `card(title, Vec<AnyElement>)` 生成 5 张卡片：① 本机数据（Paths 五个路径 + 「打开数据目录」调 `reveal_in_explorer`）；② 整理偏好（7/30/90 天保留期按钮，点击 `ops_trash::set_retention_days` 写 settings 并更新 `state.retention_days`）；③ 数据概况（照片/空间/目录/回收站/操作记录/已分析张数，全真实）；④ 隐私说明；⑤ 关于（版本与依赖）。顶部错误态横幅取 `load_error/last_error`，侧栏「设置」按钮接 `Page::Settings`。
- **M6 虚拟网格常量（src/ui/app.rs，模块内私有）**：`const GRID_COLUMNS: usize = 4`、`const GRID_TILE_HEIGHT: f32 = 150.`、`const GRID_ROW_HEIGHT: f32 = GRID_TILE_HEIGHT + 12.`；`use std::ops::Range` 供 `uniform_list` 渲染闭包签名使用。
- 其他辅助：`format_millis(i64) -> String`、`human_size(i64) -> String`（M5，GB/MB/KB 格式化）、`truncate(&str, usize) -> String`（M5，按字符截断长文本）、`subtitle_for_home(&AppState) -> String`（M5，错误/加载/正常三态）、`open_with_default(&Path)`、`reveal_in_explorer(&Path)`（windows/macos/other 三套 cfg）、`info_row`、`info_block`、`section_title`、`info_tag_section`、`dialog_btn`。
- `library` 辅助：`folder_flat_ids`、`find_folder`、`is_under`。

## 内部关键流程
1. `main.rs` 构造 Paths → `new_app_state(cx, paths)` → 内部 `db::open` + `LibraryData::reload` 加载已有索引 → `AppRoot::new` → `Root::new`。
2. 相册库「选择目录」→ `rfd::FileDialog::pick_folder` → `app.update(...start_scan(root))` → 后台线程打开独立 DB 连接执行 `scanner::scan_root`，进度经 `mpsc::channel` 回传。
3. `cx.spawn` 异步任务每 150ms 用 `gpui::background_executor().timer()` 轮询 channel：`Started/Scanning` 更新 `scan_status`，`Finished/Failed` 重新 `LibraryData::reload` 并 `cx.notify()`。
4. `page_library` 从 `data.visible_photos()` 克隆为 owned `Vec<Photo>`，按目录前缀 + 关键词过滤；M6 起用 `uniform_list("photo-virtual-grid", row_count, f)` 按 `GRID_COLUMNS=4` 张一行 lazy 渲染，闭包只构造 `range` 内的行（`GRID_ROW_HEIGHT=162px` 等高），每行 4 个 `Button` 用 `photo_thumb` 显示真实缩略图；`render_folder_tree` 递归渲染 `data.folders`（Folder 类型，id 用稳定 hash 转 usize 作 gpui ElementId），点击切换 `active_folder_id` 并重算过滤。
5. 照片点击：`ev.modifiers().control || ev.modifiers().platform`（Windows 上 platform=Win 键）时多选切 `selected`；普通点击调 `open_photo_dialog(dialog_photo.clone(), window, cx)`。弹窗左侧用 `preview_source` 选原图大图（`img().object_fit(Contain)`，`make_big` 闭包每次重绘重建），右侧分区显示基本信息/分析结果/索引字段；「打开原图」「所在文件夹」用 `open_with_default`/`reveal_in_explorer`。
6. 首页 `page_home`（M5 重写）：4 张真实统计卡来自 `data.total_count/total_bytes/root_count/visible_photos().len()`；状态横幅按「错误态（`load_error/last_error`）> 加载态（`is_scanning/is_analyzing`）> 隐藏」渲染；「🧭 整理进度」显示 `AnalyzeData` 的相似组/模糊/截图/大文件真实计数与 `analyze_status` 或 `analyzed_count`；「📋 最近整理记录」取 `OpsData.recent_ops` 前 6 条，用 `OpKind::label()` + `item_count` + `format_millis(created_at)` + `truncate(detail, 42)`，空列表显示引导空态；副标题 `subtitle_for_home` 同样分三态。
7. **M2 分析链路**：扫描 `Finished` 后自动调用 `start_analysis`（也可整理页点「重新分析」手动触发）→ 后台线程 `core::analyze::analyze_library` 写库 → `AnalyzeProgress` 经 channel 回传 → 第二个 `cx.spawn` 轮询（200ms）刷新 `analyze_status` 与 `AnalyzeData::reload`。
8. 整理页 `page_organize(state)`：6 张工具卡片徽标改真实值（前 4 个来自 `AnalyzeData::badges()`）；前 4 张 `on_click` 设 `active_tool` 进入结果列表；M4 起后 2 张（智能重命名 / 智能归档）徽标改「已就绪」并 `on_click` 跳 `Page::Rename` / `Page::Archive`（元组携带 `Option<OrganizeTool>` + `Option<Page>`，先判 page 再判 tool）；`render_tool_result` 按工具分发到相似组列表 / 通用照片列表；结果项与相似组卡片用 `photo_thumb` 显示真实缩略图，且 `on_click` 打开详情弹窗。侧栏「智能整理」徽标由硬编码 12 改为 `groups+blurry+screenshots+large` 的真实合计。
9. **M3 批量删除**：相册库工具栏「删除到回收站」→ `AppState::batch_trash` → `selected_photo_ids()` 映射多选 → `core::ops::trash::trash_photos`（同步，文件移动 + DB 事务）→ `reload_all()` 刷新并清空选中，`ops_status` 显示数量/占用空间；撤销走 `undo_trash()`。
10. **M3 批量重命名**：工具栏「重命名」跳 `Page::Rename` → `page_rename` 展示 5 个预设模板卡片（点击改 `rename_pattern`）+ 用当前 pattern/prefix 经 `ops_rename::preview` 计算前 5 条实时预览 → 「执行重命名」调 `batch_rename()`（execute）→ 「撤销上次重命名」调 `undo_rename()`。模板与前缀存于 AppState（无文本输入框，避免 dialog 中 InputState 的 window 生命周期复杂度）。
11. **M3 回收站页**：`page_trash` 从 `OpsData.trash` 渲染列表（文件名/大小/删除距今/恢复按钮）→ 恢复调 `ops_trash::restore_items`；底部「撤销上次删除」「清理 30 天前」「清空回收站」分别调 `undo_trash`/`purge_expired(conn,30)`/`empty_trash`；侧栏回收站徽标用 `ops_data.trash_count` 真实值。行首 44×44 缩略图用 `photo_thumb`，点击打开详情；调用前把 `photo.path` 重定向到 `TrashItem.trash_path`（原图已移动）。
12. **图片渲染（16:35 新增）**：所有照片展示位（相册库格子 / 整理页结果列表 / 相似组卡片 / 回收站行）统一走 `photo_thumb(&Photo)`；`gpui::img(PathBuf)` 在 0.2.2 走 `ImageSource::Resource(Resource::Path)` → `ImageAssetLoader` 用 `fs::read` 读本地文件 + `image` crate 解码，**不经过只认图标的 `Assets`**；`with_fallback`（解码失败）与 `with_loading`（超过 LOADING_DELAY）均降级到 `photo_placeholder` 色块。扩展名不在 `Img::extensions()` 内（HEIC/RAW 等）由 `is_decodable_image` 拦下，不喂给 gpui。
13. **M4 归档链路**：`new_app_state` 从 `settings` 读 `archive.dest/rule/mode` → `refresh_archive_plan()` 调 `core::ops::archive::plan` 生成只读计划（含 `tree` 完整目录树）→ `page_archive` 展平树渲染预览 + 统计。切换规则/模式/范围或 `rfd` 选目标目录时，先更新 AppState + `persist_archive_settings()` 再重算预览。「执行归档」先再次 `refresh_archive_plan()` 保证预览与落盘一致 → `ops_archive::execute` → `reload_all()` + 重算预览；「撤销上次归档」调 `ops_archive::undo`（按 `ArchiveMove`/`ArchiveCopy` 分支）。归档状态提示复用 `ops_status`。

14. **M5 首页数据链路**：`new_app_state` 加载三份快照 → `page_home` 一次性提取 owned 值 → 统计卡 / 进度区 / 最近操作列表全部读索引与 `op_log`，无任何模拟数字；`subtitle_for_home` 与状态横幅反映实时扫描/分析进度与失败原因。
15. **M5 保留期设置链路**：设置页按钮 → `db::open` + `ops_trash::set_retention_days`（写 `settings.trash.retention_days`）→ 更新 `AppState.retention_days`；回收站页 `purge` 按钮标签、`purge_expired(&conn, s.retention_days)`、结果文案与说明文字均跟随该值，不再硬编码 30。
16. **M5 错误/加载态**：`start_scan`/`start_analysis` 的 `Finished` 清 `last_error`、`Failed` 写 `last_error`；`reload_all` 成功清 `load_error`、失败写入；首页与设置页用红色横幅展示，加载中用金色横幅展示后台状态文字。
17. **M5 窗口与图标资源**：`main.rs` 的 `WindowOptions.window_min_size = Some(Size::new(px(960.), px(640.)))`，1200×780 仅作初始 bounds；`AppRoot::render` 的 `windowcard` 改 `.size_full()` 跟随窗口；`src/ui/assets.rs::Assets::load` 登记 `assets/icons/` 下全部 47 个 SVG（gpui-component 内部组件也按 `IconName::path()` 请求资源）。

18. **M6 虚拟网格链路**：`page_library` 把 `visible_photos()` 克隆为 owned `photos`，`total_visible`/`row_count` 提前算好；`uniform_list` 的 `Fn(Range<usize>, &mut Window, &mut App) -> Vec<AnyElement>` 闭包捕获 `photos`/`selected`/`app_for_list`，按 `row * GRID_COLUMNS + column` 还原全局索引 `idx`（与 `selected` 的索引口径一致），因此多选与点击详情行为与改造前完全相同；行高固定 `GRID_ROW_HEIGHT = 150 + 12`，uniform_list 测量首行后按等高排布，5000 张只创建视口内约十余行元素。

## 依赖关系
- 依赖的外部模块：`core-model`（Photo/Folder/PhotoGroup/OpLog/TrashItem/OpKind）、`core-scanner`（db/scanner/settings）、`core-analyze`（AnalyzeProgress/analyze_library）、`core-ops`（trash/rename/archive 批量操作）、`core-config`（Paths）、`rfd`（目录对话框，扫描与归档目标共用）、gpui/gpui-component、icon-assets。
- 被谁依赖：`main.rs` 组合根。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` → `render_page`（M5 新增 `Page::Settings` 与 `page_settings`）。
- 新增状态态：错误/加载信息统一写 `AppState.last_error/load_error`，页面读同一份状态，勿各页自定义错误文案。
- 新增设置项：`page_settings` 加卡片 → core 提供 get/set（复用 `db::get_setting/set_setting`）→ 在 `tests/settings_test.rs` 补默认值/脏值回退用例。
- 新增图标：SVG 放入 `assets/icons/` 后必须在 `Assets::load` 登记，缺登记会让 gpui-component 图标空白。
- 虚拟网格：列数/行高常量在 app.rs 顶部；改列数需同步 `row_count` 与 `idx` 计算，行必须等高，否则 uniform_list 测量会错位。
- 改扫描交互：调整 `start_scan` 的后台线程与 channel 轮询逻辑。
- 缩略图显示：已统一在 `photo_thumb`（app.rs）；换图源/尺寸改 `thumbnail_source`/`preview_source` 与父容器尺寸；新增展示位直接复用 `photo_thumb(&Photo)`。详情大图改 `make_big` 内的 `ObjectFit` 与 `preview_source`。
- 分析结果交互：在 `render_photo_list`/`render_similar_groups` 增加「保留最佳」/删除动作（删除可复用 `AppState::batch_trash` 的选中 id 路径，但当前结果列表项未接入 `selected`）。
- 归档规则/范围：加规则改 `core-ops::archive::ArchiveRule`（UI 遍历 `ALL` 自动跟随）；加范围维度在 `archive_ids()` 扩展并给 `page_archive` 加切换控件。
- 重命名交互增强：若要自由输入模板，需引入 `InputState::new(window, cx)` 实体并在 dialog 中持有 window；当前用预设模板规避。
- 批量操作进度：当前删除/重命名同步执行（适合个人库规模）；量大时可参照扫描/分析改后台线程 + channel。

## 已知风险 / 易错点
- gpui 0.2.2 `cx.spawn` 异步闭包签名是 `FnOnce(WeakEntity<T>, &mut AsyncApp)`；异步上下文用全局 `gpui::background_executor()`，无 `cx.background_executor()`。
- gpui ElementId 不接受 `String`，目录树用 `stable_id()` 转 usize；不同目录 hash 冲突概率极低但非零。
- `state` 从 `self.app.read(cx)` 借用期间不能再可变借用 cx，故 page_* 函数开头即把数据提取为 owned 值。
- `open_dialog` 闭包是 `Fn`（非 FnOnce），捕获的 String 必须 `.clone()` 后再传入内部函数。
- rfd 目录对话框用同步 `pick_folder()`（会短暂阻塞 UI），换异步需处理 `AsyncApp` 生命周期。
- gpui 0.2.2 中 `AnyElement` 非 struct，函数返回统一用 `impl IntoElement + 'static`，分支内 `.into_any_element()` 后保持同型。
- `img(PathBuf)` 走 `Resource::Path` 的 `fs::read`，路径必须是本地绝对路径；`Assets`（src/ui/assets.rs）只服务 `Resource::Embedded` 图标，不参与照片加载。
- 图片是异步 asset cache：`with_fallback` 只在解码失败时显示、`with_loading` 只在超时未完成显示，两者都要给，否则加载中白屏；本地小 jpg/png 通常瞬间完成。
- `open_dialog` 渲染闭包是 `Fn`（每次重绘调用），大图元素必须每次重建（现用 `make_big: impl Fn() -> AnyElement`）；不能把单个 `AnyElement` move 进闭包一次用完。
- 详情弹窗大图默认用原图；回收站项原图已移动，若不重定向 `photo.path` 到 `trash_path`，「打开原图」会指向失效路径。
- 归档目标目录不要落在扫描根目录内，否则下次扫描会把归档副本当新照片重复索引；UI 默认目标是「扫描根目录的同级 / 归档」，用户可用 rfd 覆盖。
- 归档执行前必须重算 `archive_plan`，否则用户改了规则/范围后落盘与预览不一致；`execute_archive()` 已内置刷新。
- 不支持格式（HEIC/RAW 等）由 `is_decodable_image` 拦下：此时网格仍可用缩略图缓存（扫描已生成 jpg），详情弹窗经 `preview_source` 回退到缩略图。
- 首页/设置页错误态只显示最近一条；`last_error` 在下次后台任务成功时清除，历史错误不保留（离线可用，错误不阻断功能）。
- `windowcard` 用 `.size_full()` 后网格列数仍固定 4 列（gpui 0.2.2 无响应式断点），窄窗口靠滚动查看；`window_min_size` 保证侧栏与内容区不挤压。
- `uniform_list` 渲染闭包是 `Fn` 且 `'static`：捕获的 `photos` 被 move 后工具栏不能再借用，必须先存 `total_visible`；循环变量不可命名 `col`（会遮蔽 `col()` 配色函数，编译报 E0618）。
- uniform_list 依赖「所有 item 等高」：行 div 显式 `.h(px(GRID_ROW_HEIGHT))`，tile 固定 `GRID_TILE_HEIGHT`，底部 12px 用 `.pb()` 留白，勿改成自适应高度。

## 变更记录
- 2026-11-19：M6 相册库网格改 `uniform_list` 虚拟滚动（每行 4 张 / 等高 162px / 只渲染视口内行），新增 `GRID_COLUMNS`/`GRID_TILE_HEIGHT`/`GRID_ROW_HEIGHT` 常量与 `std::ops::Range` 导入；多选与详情点击保持原全局索引口径。刷新 hash。
- 2026-11-18 17:40：M5 首页改真实最近操作/整理进度/加载·空·错误态，新增 `Page::Settings` 与 `page_settings`（本机路径/保留期/数据概况/隐私/关于），AppState 增加 3 个状态字段并维护后台失败信息，回收站清理按钮跟随设置值；`windowcard` 改 `.size_full()` 配合 main.rs `window_min_size`；`Assets::load` 登记全部 47 个 SVG；新增 `human_size`/`truncate` helper。刷新 hash。
- 2026-11-18 16:58：M4 新增 `Page::Archive` + 侧栏导航 + `page_archive`（4 规则 / Move+Copy / rfd 目标目录 / 范围切换 / 完整目录树预览 / 统计 / 执行 / 撤销），AppState 增加归档配置与计划快照并持久化到 settings；整理页后 2 张卡片改为可跳转、侧栏「智能整理」徽标改真实合计；新增 `flatten_archive_node`/`archive_stat_row` helper。刷新 hash。
- 2026-11-18 16:35：修复「能看到数量但看不到缩略图、点击无原图/信息」——UI 从未调用图片渲染（全为色块）。新增图片渲染 helper 组（`thumbnail_source`/`preview_source`/`photo_thumb`/`photo_placeholder` 等），相册库格子/整理结果/相似组/回收站全部接入 gpui `img(PathBuf)` 真实缩略图；`open_photo_dialog` 改接受 owned Photo，弹窗升级为原图大图 + 基本信息/分析结果/索引分区 + 打开原图/所在文件夹按钮；整理结果与回收站项也可点开详情。刷新 hash。
- 2026-11-18 16:05：M3 相册库工具栏接批量删除/重命名，新增重命名页（预设模板+预览+撤销）与回收站页（恢复/清空/清理/撤销），新增 `ui/ops.rs` 适配层、AppState 批量操作方法，移除占位页/按钮，刷新 hash。
- 2026-11-18 15:40：M2 整理页工具卡片接真实分析结果、新增后台分析调度与结果列表，新增 `ui/analyze.rs` 适配层，刷新 hash。
- 2026-11-18 15:10：M1 数据流改为真实索引；新增 `ui/library.rs` 适配层，接入目录选择/后台扫描/目录过滤/真实统计；整模块重写。
- 2026-11-16 17:35：M0 从 main.rs 迁入 UI 代码。
