# 模块：ui-desktop

> 生成时间：2026-11-16 17:35 +0800
> 覆盖范围：src/ui/app.rs, src/ui/assets.rs, src/ui/mod.rs（按路径排序）
> 覆盖 hash：61a24377e14e（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面端 UI 层，实现窗口标题栏、深色侧栏、首页、智能整理页、相册库页、重命名/回收站占位页与照片详情弹窗。M0 结构性变更后，所有渲染代码从 `main.rs` 迁入 `src/ui/`，纯数据改由 `core::model` 提供。数据仍为静态模拟，尚未接入真实扫描或图像算法。

## 对外接口
- `pub struct AppState`（src/ui/app.rs）：应用全局状态，字段私有。
- `pub fn new_app_state(cx: &mut App) -> Entity<AppState>`：创建初始状态供组合根调用；初始页 `Page::Library`。
- `pub struct AppRoot`（src/ui/app.rs）：根渲染组件。
- `pub fn AppRoot::new(app: Entity<AppState>) -> Self`：构造根组件。
- `impl Render for AppRoot { fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement }`：根布局渲染。
- `pub struct Assets`（src/ui/assets.rs）+ `impl AssetSource for Assets`：`fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>`；`fn list(&self, _path: &str) -> Result<Vec<SharedString>>`。编译期嵌入 22 个图标 ID。

## 内部关键流程
1. 组合根 `main.rs` 调用 `new_app_state(cx)` → `AppRoot::new(state)` → `Root::new(view, window, cx)`。
2. `AppRoot::render` 拼装 `titlebar`、`sidebar`（`AppRoot::sidebar`）、`content`（`AppRoot::render_page`），外层固定 1200×780 窗口卡片。
3. `AppRoot::sidebar` 为 5 个导航项生成 `Button::new(...).on_click`，点击 `app.update` 修改 `AppState.page` 并 `cx.notify()`；设置按钮 `on_click` 为空闭包。
4. `AppRoot::render_page` 按 `Page` 分发：`Home`/`Organize`/`Library` 分别对应 `page_home`/`page_organize`/`page_library`，`Rename`/`Trash` → `page_placeholder`。
5. `page_library` → `core::model::folder_defs()` + `render_folder_tree` 生成目录面板；`core::model::PHOTOS` 枚举生成 4 列照片网格。点击照片含 Ctrl/Platform 修饰键则切换 `AppState.selected`，否则 `open_photo_dialog`。
6. `render_folder_tree` 递归输出 `AnyElement`，节点点击更新 `active_folder` 并切换 `folder_expanded[expandable_id(...)]`；`expandable_id` 由 `core::model` 提供。
7. `open_photo_dialog` → `window.open_dialog`，左侧色块模拟预览，右侧显示文件名/日期/尺寸/大小/设备/地点/标签，底部收藏/标签/移动/删除按钮；`on_ok` 仅 `window.close_dialog`。

## 依赖关系
- 依赖的外部模块：`akashic::core::model`（`folder_defs`/`PHOTOS`/`FolderNode`/`expandable_id`）、gpui、gpui-component、assets/icons（见 icon-assets 模块）。
- 被谁依赖：`main.rs`（组合根调用 `new_app_state`/`AppRoot`/`Assets`）。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` 增加导航项 → `render_page` 增加分支。
- 接入真实相册：替换 `core::model` 的静态数据，补充扫描、缩略图加载与筛选逻辑。
- 让工具卡片/操作按钮生效：为 `page_organize` 工具卡片、`header_btn`、`open_photo_dialog` 底部按钮补 `on_click`。
- 调整视觉：文件头部颜色常量、`page_*` 函数尺寸间距；同步检查 design-prototype 模块。

## 已知风险 / 易错点
- 全为模拟数据：12 张照片、目录数量、首页统计（8,432 张 / 42.6 GB）与侧栏徽标均为硬编码。
- 目录点击只改变高亮与展开状态，不过滤右侧照片网格；重命名页、回收站页仅为占位。
- 详情弹窗收藏/标签/移动/删除按钮无实际效果，`on_ok` 只关闭窗口。
- `Assets::load` 显式列出 22 个图标 ID，新增 SVG 不登记则运行时无法加载（当前未用 book-open/search/close/plus）。
- `AppState` 必须为 `pub`（出现在公共函数签名中），否则 `main.rs` 无法编译。

## 变更记录
- 2026-11-16 17:35：M0 结构性变更，`main.rs` 的 UI 代码迁入 `src/ui/{app,assets,mod}.rs`，纯数据迁至 `core::model`；整模块按模板重写。
