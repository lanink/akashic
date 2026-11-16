# 模块：ui-desktop

> 生成时间：2026-11-16 16:08:19 +0800
> 覆盖范围：src/lib.rs, src/main.rs（按路径排序）
> 覆盖 hash：3de2319b1dca（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
基于 gpui / gpui-component 的桌面端 UI 原型，实现窗口标题栏、深色侧栏、首页、智能整理页、相册库页、重命名/回收站占位页与照片详情弹窗。所有照片数据来自静态常量，尚未接入文件系统扫描或图像算法。

## 对外接口
- `pub fn add(left: u64, right: u64) -> u64`（src/lib.rs）：示例库函数，仅被单元测试 `it_works` 调用。
- `fn main()`（src/main.rs）：二进制入口，初始化 gpui、gpui-component，打开 1200×780 窗口。
- `fn col(hex: u32) -> Hsla`、`fn rgba(r: u32, g: u32, b: u32, a: f32) -> Hsla`：设计稿色值转换。
- `fn photo_block_bg(hue: f32) -> Hsla`、`fn photo_icon_bg(hue: f32) -> Hsla`：模拟缩略图配色。
- `fn folder_defs() -> Vec<FolderNode>`：构造静态相册目录树（日本旅行 / 生日派对 / 四姑娘山 / 2024年 / 2023年及子目录）。
- `impl AppState { fn new(cx: &mut App) -> Entity<Self> }`：创建全局状态；初始页为 `Page::Library`。
- `impl Render for AppRoot { fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement }`：根布局渲染。
- `fn render_folder_tree(folders: &[FolderNode], state: &AppState, base: usize, app: Entity<AppState>) -> impl IntoElement + 'static`：递归渲染目录树。
- `fn expandable_id(folders: &[FolderNode], target: usize) -> Option<usize>`：深度优先扁平索引到可展开节点下标的映射。
- `fn open_photo_dialog(app: &Entity<AppState>, idx: usize, window: &mut Window, cx: &mut App)`：打开照片详情对话框。
- `fn header_btn(icon: IconName, label: &'static str, color: u32) -> Button`、`fn dialog_btn(icon: IconName, label: &'static str, color: u32) -> Button`、`fn info_tag_section(title: &'static str, tags: Vec<AnyElement>) -> AnyElement`：按钮与标签构件。
- `struct Assets` + `impl AssetSource for Assets { fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>; fn list(&self, _path: &str) -> Result<Vec<SharedString>> }`：编译期嵌入图标资源。

## 内部关键流程
1. `main` → `gpui::Application::new().with_assets(Assets)` → `app.run` → `gpui_component::init(cx)` → `cx.spawn` 内 `cx.open_window` → `AppState::new` → `AppRoot::new` → `Root::new(view, window, cx)`。
2. `AppRoot::render` 拼装 `titlebar`（标题 + 本地/离线状态）、`sidebar`（`AppRoot::sidebar`）和 `content`（`AppRoot::render_page`），外层固定 1200×780 窗口卡片。
3. `AppRoot::sidebar` 为 5 个导航项生成 `Button::new(...).on_click`，点击后 `app.update` 修改 `AppState.page` 并 `cx.notify()`；设置按钮的 `on_click` 为空闭包。
4. `AppRoot::render_page` 按 `Page` 分发：`Home` → `page_home`，`Organize` → `page_organize`，`Library` → `page_library`，`Rename` / `Trash` → `page_placeholder`。
5. `page_library` → `folder_defs()` + `render_folder_tree` 生成目录面板；`PHOTOS` 枚举生成 4 列照片网格。点击照片时若含 Ctrl/Platform 修饰键则切换 `AppState.selected`，否则调用 `open_photo_dialog`。
6. `render_folder_tree` 递归输出 `AnyElement`；节点点击更新 `active_folder` 并切换 `folder_expanded[expandable_id(...)]`。
7. `open_photo_dialog` → `window.open_dialog`，左侧为色块模拟预览（明确标注“模拟预览 · 实际为照片缩略图”），右侧显示文件名/日期/尺寸/大小/设备/地点/标签，底部为收藏、标签、移动、删除按钮；`on_ok` 仅 `window.close_dialog`。

## 依赖关系
- 依赖的外部模块：gpui、gpui-component、anyhow、assets/icons（见 icon-assets 模块）。
- 被谁依赖：无（二进制 crate）；src/lib.rs 为独立示例库，未被 main.rs 引用。

## 常见改动点
- 新增页面：扩展 `Page` 枚举 → `sidebar` 增加导航项 → `render_page` 增加分支。
- 接入真实相册：替换 `PHOTOS` / `PhotoDef` / `folder_defs()` 静态数据，补充扫描、缩略图加载与筛选逻辑。
- 让工具卡片/操作按钮生效：为 `page_organize` 的工具卡片、`header_btn` 和 `open_photo_dialog` 底部按钮补充 `on_click`。
- 调整视觉：文件头部颜色常量、`page_*` 函数中的尺寸与间距；同步检查 design-prototype 模块。

## 已知风险 / 易错点
- 全为模拟数据：12 张照片、目录数量、首页统计（8,432 张 / 42.6 GB）和侧栏徽标均为硬编码，与真实相册无关。
- 目录点击只改变高亮与展开状态，不会过滤右侧照片网格；重命名页、回收站页仅为占位。
- 照片详情中的收藏/标签/移动/删除按钮没有实际效果，`on_ok` 也只是关闭窗口。
- Cargo.toml 声明了 dssim、image、imageflow_core/helpers/types，但当前源码未调用，算法能力尚未实现。
- `Assets::load` 显式列出 22 个图标 ID；新增 SVG 后若不在此登记，运行时无法加载（当前未使用列表中的 book-open/search/close/plus）。
- `src/lib.rs` 的 `add` 是模板样例，与产品功能无关。
