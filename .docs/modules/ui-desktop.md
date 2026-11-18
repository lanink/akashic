# 模块：ui-desktop

> 生成时间：2026-11-18 16:35 +0800
> 覆盖范围：src/ui/analyze.rs, src/ui/app.rs, src/ui/assets.rs, src/ui/library.rs, src/ui/mod.rs, src/ui/ops.rs（按路径排序）
> 覆盖 hash：0c2dd63c5d2a（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面 UI 层。M1 起相册库/首页读真实 SQLite 索引（目录选择/后台扫描/目录过滤/多选/照片详情）；M2 智能整理页 4 个分析工具接入分析与后台调度；M3 新增批量删除到回收站、批量重命名（预设模板+预览+一次撤销）、回收站页（恢复/清空/30 天清理/撤销），删除绝不永久丢失原文件。本次修复把所有照片展示位接入真实缩略图（gpui `img(PathBuf)`），并把详情弹窗升级为「原图大图 + 完整图像信息」。

## 对外接口
- `pub struct AppState`：字段 `page/paths/data/active_folder_id/folder_expanded/selected/scan_status/is_scanning/progress_rx/analyze_data/analyze_rx/is_analyzing/analyze_status/active_tool/ops_data/ops_status/rename_pattern/rename_prefix/last_rename_op/last_trash_op`（私有）。
- `pub enum OrganizeTool`：`Similar/Blurry/Screenshot/Large`（整理页当前工具）。
- `pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState>`：创建初始状态并从索引加载 LibraryData + AnalyzeData + OpsData。
- `AppState::visible_count() -> usize`、`start_scan(root, cx)`、`start_analysis(cx)`：分别启动后台扫描与分析。
- `AppState` 私有方法（M3）：`reload_all()`（刷新三份快照）、`selected_photo_ids() -> Vec<i64>`（多选索引→照片 id）、`batch_trash()`、`batch_rename()`、`undo_rename()`、`undo_trash()`。
- `pub struct AppRoot` + `AppRoot::new(app: Entity<AppState>)` + `impl Render`。
- `pub struct Assets` + `impl AssetSource`（src/ui/assets.rs，编译期嵌入图标）。
- `LibraryData`（src/ui/library.rs）：`reload(conn)`、`visible_photos()`、`thumbnail_for(&Photo)`；字段见源码。
- `AnalyzeData`（src/ui/analyze.rs，M2）：`reload(conn) -> Result<Self>`、`badges() -> [String;4]`、`is_ready() -> bool`；字段 `groups/blurry/screenshots/large/analyzed_count/loaded`。
- `OpsData`（src/ui/ops.rs，M3）：`reload(conn) -> Result<Self>`、`size_label() -> String`；字段 `trash: Vec<TrashItem>/trash_count/trash_bytes/recent_ops: Vec<OpLog>/loaded`。
- **图片渲染 helper（src/ui/app.rs，本次新增，均为模块内私有 fn）**：`is_decodable_image(&Path) -> bool`（扩展名是否落在 `gpui::Img::extensions()`）；`thumbnail_source(&Photo) -> Option<PathBuf>`（缩略图缓存优先，缺失回退原图）；`preview_source(&Photo) -> Option<PathBuf>`（原图优先，不可解码回退缩略图）；`photo_placeholder(hue: f32, location: &str, name: &str) -> AnyElement`（色块降级）；`photo_thumb(&Photo) -> AnyElement`（`img()` + `ObjectFit::Cover` + fallback/loading 降级）。
- `fn open_photo_dialog(photo: Photo, window: &mut Window, cx: &mut App)`：改为接受 owned Photo（不再依赖 `AppState`/visible 索引），相册库/整理结果/相似组/回收站均可调用；弹窗左侧原图大图（`ObjectFit::Contain`），右侧分区显示基本信息/分析结果/索引字段，「打开原图」「所在文件夹」调系统命令。
- 其他辅助：`format_millis(i64) -> String`、`open_with_default(&Path)`、`reveal_in_explorer(&Path)`（windows/macos/other 三套 cfg）、`info_row`、`info_block`、`section_title`、`info_tag_section`、`dialog_btn`。
- `library` 辅助：`folder_flat_ids`、`find_folder`、`is_under`。

## 内部关键流程
1. `main.rs` 构造 Paths → `new_app_state(cx, paths)` → 内部 `db::open` + `LibraryData::reload` 加载已有索引 → `AppRoot::new` → `Root::new`。
2. 相册库「选择目录」→ `rfd::FileDialog::pick_folder` → `app.update(...start_scan(root))` → 后台线程打开独立 DB 连接执行 `scanner::scan_root`，进度经 `mpsc::channel` 回传。
3. `cx.spawn` 异步任务每 150ms 用 `gpui::background_executor().timer()` 轮询 channel：`Started/Scanning` 更新 `scan_status`，`Finished/Failed` 重新 `LibraryData::reload` 并 `cx.notify()`。
4. `page_library` 从 `data.visible_photos()` 渲染网格（按目录前缀 + 关键词过滤）；`render_folder_tree` 递归渲染 `data.folders`（Folder 类型，id 用稳定 hash 转 usize 作 gpui ElementId），点击切换 `active_folder_id` 并重算过滤。
5. 照片点击：`ev.modifiers().control || ev.modifiers().platform`（Windows 上 platform=Win 键）时多选切 `selected`；普通点击调 `open_photo_dialog(dialog_photo.clone(), window, cx)`。弹窗左侧用 `preview_source` 选原图大图（`img().object_fit(Contain)`，`make_big` 闭包每次重绘重建），右侧分区显示基本信息/分析结果/索引字段；「打开原图」「所在文件夹」用 `open_with_default`/`reveal_in_explorer`。
6. 首页 `page_home` 从 `data.total_count/total_bytes/root_count` 显示真实统计；无索引时展示引导空态。
7. **M2 分析链路**：扫描 `Finished` 后自动调用 `start_analysis`（也可整理页点「重新分析」手动触发）→ 后台线程 `core::analyze::analyze_library` 写库 → `AnalyzeProgress` 经 channel 回传 → 第二个 `cx.spawn` 轮询（200ms）刷新 `analyze_status` 与 `AnalyzeData::reload`。
8. 整理页 `page_organize(state)`：6 张工具卡片徽标改真实值（前 4 个来自 `AnalyzeData::badges()`）；前 4 张 `on_click` 设 `active_tool` 进入结果列表；`render_tool_result` 按工具分发到相似组列表 / 通用照片列表；结果项与相似组卡片用 `photo_thumb` 显示真实缩略图，且 `on_click` 打开详情弹窗。
9. **M3 批量删除**：相册库工具栏「删除到回收站」→ `AppState::batch_trash` → `selected_photo_ids()` 映射多选 → `core::ops::trash::trash_photos`（同步，文件移动 + DB 事务）→ `reload_all()` 刷新并清空选中，`ops_status` 显示数量/占用空间；撤销走 `undo_trash()`。
10. **M3 批量重命名**：工具栏「重命名」跳 `Page::Rename` → `page_rename` 展示 5 个预设模板卡片（点击改 `rename_pattern`）+ 用当前 pattern/prefix 经 `ops_rename::preview` 计算前 5 条实时预览 → 「执行重命名」调 `batch_rename()`（execute）→ 「撤销上次重命名」调 `undo_rename()`。模板与前缀存于 AppState（无文本输入框，避免 dialog 中 InputState 的 window 生命周期复杂度）。
11. **M3 回收站页**：`page_trash` 从 `OpsData.trash` 渲染列表（文件名/大小/删除距今/恢复按钮）→ 恢复调 `ops_trash::restore_items`；底部「撤销上次删除」「清理 30 天前」「清空回收站」分别调 `undo_trash`/`purge_expired(conn,30)`/`empty_trash`；侧栏回收站徽标用 `ops_data.trash_count` 真实值。行首 44×44 缩略图用 `photo_thumb`，点击打开详情；调用前把 `photo.path` 重定向到 `TrashItem.trash_path`（原图已移动）。
12. **图片渲染（本次新增）**：所有照片展示位（相册库格子 / 整理页结果列表 / 相似组卡片 / 回收站行）统一走 `photo_thumb(&Photo)`；`gpui::img(PathBuf)` 在 0.2.2 走 `ImageSource::Resource(Resource::Path)` → `ImageAssetLoader` 用 `fs::read` 读本地文件 + `image` crate 解码，**不经过只认图标的 `Assets`**；`with_fallback`（解码失败）与 `with_loading`（超过 LOADING_DELAY）均降级到 `photo_placeholder` 色块。扩展名不在 `Img::extensions()` 内（HEIC/RAW 等）由 `is_decodable_image` 拦下，不喂给 gpui。

## 依赖关系
- 依赖的外部模块：`core-model`（Photo/Folder/PhotoGroup/OpLog/TrashItem）、`core-scanner`（db/scanner）、`core-analyze`（AnalyzeProgress/analyze_library）、`core-ops`（trash/rename 批量操作）、`core-config`（Paths）、`rfd`（目录对话框）、gpui/gpui-component、icon-assets。
- 被谁依赖：`main.rs` 组合根。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` → `render_page`。
- 改扫描交互：调整 `start_scan` 的后台线程与 channel 轮询逻辑。
- 缩略图显示：已统一在 `photo_thumb`（app.rs）；换图源/尺寸改 `thumbnail_source`/`preview_source` 与父容器尺寸；新增展示位直接复用 `photo_thumb(&Photo)`。详情大图改 `make_big` 内的 `ObjectFit` 与 `preview_source`。
- 分析结果交互：在 `render_photo_list`/`render_similar_groups` 增加「保留最佳」/删除动作（删除可复用 `AppState::batch_trash` 的选中 id 路径，但当前结果列表项未接入 `selected`）。
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
- 不支持格式（HEIC/RAW 等）由 `is_decodable_image` 拦下：此时网格仍可用缩略图缓存（扫描已生成 jpg），详情弹窗经 `preview_source` 回退到缩略图。

## 变更记录
- 2026-11-18 16:35：修复「能看到数量但看不到缩略图、点击无原图/信息」——UI 从未调用图片渲染（全为色块）。新增图片渲染 helper 组（`thumbnail_source`/`preview_source`/`photo_thumb`/`photo_placeholder` 等），相册库格子/整理结果/相似组/回收站全部接入 gpui `img(PathBuf)` 真实缩略图；`open_photo_dialog` 改接受 owned Photo，弹窗升级为原图大图 + 基本信息/分析结果/索引分区 + 打开原图/所在文件夹按钮；整理结果与回收站项也可点开详情。刷新 hash。
- 2026-11-18 16:05：M3 相册库工具栏接批量删除/重命名，新增重命名页（预设模板+预览+撤销）与回收站页（恢复/清空/清理/撤销），新增 `ui/ops.rs` 适配层、AppState 批量操作方法，移除占位页/按钮，刷新 hash。
- 2026-11-18 15:40：M2 整理页工具卡片接真实分析结果、新增后台分析调度与结果列表，新增 `ui/analyze.rs` 适配层，刷新 hash。
- 2026-11-18 15:10：M1 数据流改为真实索引；新增 `ui/library.rs` 适配层，接入目录选择/后台扫描/目录过滤/真实统计；整模块重写。
- 2026-11-16 17:35：M0 从 main.rs 迁入 UI 代码。
