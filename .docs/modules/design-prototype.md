# 模块：design-prototype

> 生成时间：2026-11-16 16:08:19 +0800
> 覆盖范围：akashic-ds.html（按路径排序）
> 覆盖 hash：d32b2f6af661（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
单文件 HTML 交互设计稿，用 CSS 变量与原生 JS 还原 Akashic 桌面窗口的视觉规范、5 个页面与照片详情弹窗，作为 gpui 实现的参照。

## 对外接口
- `function switchPage(pageId)`：按 `page-${pageId}` 切换当前显示页面。
- `function switchLibraryFolder(el, label)`：切换相册库左侧目录选中态并更新标题。
- `function toggleTree(el)`：展开/收起目录树子级。
- `function renderGrid(photos)`：根据照片数组生成相册库网格。
- `function updateSelectionBar()`：更新已选照片数量与操作栏。
- `function openModal(index)` / `function closeModal()` / `function navigatePhoto(direction)`：照片详情弹窗的打开、关闭与前后切换。
- 页面节点 ID：`page-home`、`page-organize`、`page-library`、`page-rename`、`page-trash`、`photoModal`。

## 内部关键流程
1. `:root` 定义品牌色（--primary #1A2634、--gold #C9A96E、--bg-warm #F5F2ED 等）、圆角、阴影与过渡变量；`.window` 固定 1200×780。
2. 标题栏 `.titlebar`、侧栏导航与 `.page` 容器构成主体；`page-library` 默认 `active`。
3. 脚本中 `const photoData = [...]` 提供静态照片，`renderGrid(photos)` 动态创建网格元素并绑定点击。
4. 点击照片 → `openModal(index)` → 填充 `#modalFilename`、`#infoFilename`、`#infoDate`、`#infoSize`、`#infoFileSize`、`#infoDevice`、`#infoLocation`、`#infoTags` → 显示 `#photoModal`；`navigatePhoto(direction)` 修改 `currentIndex`。
5. Ctrl/点击多选 → `selectedIds` → `updateSelectionBar()`；目录点击 → `switchLibraryFolder` / `toggleTree`。

## 依赖关系
- 依赖的外部模块：CDN 上的 Font Awesome 6.0.0-beta3（图标字体）。
- 被谁依赖：ui-desktop 模块的颜色常量与布局参照；不参与 Rust 构建。

## 常见改动点
- 调整视觉规范：修改 `:root` 变量与 `.window` / `.titlebar` / `.page` 样式，再同步 ui-desktop 的 `const` 颜色。
- 修改页面结构：编辑对应 `#page-*` 区块；新增页面需同步 JS `switchPage` 与 Rust `Page` 枚举。
- 修改模拟数据：编辑脚本中的 `photoData`；同步考虑 Rust 侧 `PHOTOS`。

## 已知风险 / 易错点
- 依赖外部 CDN 加载图标字体，离线打开时图标可能缺失。
- `page-rename` 与 `page-trash` 在 HTML 中也是占位内容，未体现 PRD 中的模板配置与回收站恢复流程。
- 弹窗图片为 emoji / 色块模拟，不是真实照片。
- 设计稿与 Rust 实现是两套独立代码，改动任一侧都不会自动同步。
