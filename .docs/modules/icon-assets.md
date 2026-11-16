# 模块：icon-assets

> 生成时间：2026-11-16 16:08:19 +0800
> 覆盖范围：assets/icons/*.svg（47 个，按路径排序）
> 覆盖 hash：4f60f3a7bd02（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
存放 Lucide 线性 SVG 图标，由 Rust 侧 `Assets` 实现在编译期嵌入，供 gpui-component 的导航与按钮使用。

## 对外接口
无代码接口。资源 ID 到文件的映射在 ui-desktop 模块 `impl AssetSource for Assets` 的 `load` 中定义，当前登记 22 个：
`icons/layout-dashboard.svg`、`icons/settings-2.svg`、`icons/gallery-vertical-end.svg`、`icons/case-sensitive.svg`、`icons/delete.svg`、`icons/settings.svg`、`icons/copy.svg`、`icons/eye-off.svg`、`icons/frame.svg`、`icons/file.svg`、`icons/folder-open.svg`、`icons/book-open.svg`、`icons/star.svg`、`icons/search.svg`、`icons/close.svg`、`icons/chevron-right.svg`、`icons/check.svg`、`icons/info.svg`、`icons/circle-user.svg`、`icons/circle-check.svg`、`icons/redo.svg`、`icons/plus.svg`。
`AssetSource::list` 当前返回空列表。

## 内部关键流程
1. `main` 调用 `gpui::Application::new().with_assets(Assets)` 注册资源源。
2. `Assets::load(path)` 通过 `include_bytes!("../assets/icons/<name>.svg")` 在编译期读取文件，运行时返回 `Cow::Owned(bytes.to_vec())`。
3. ui-desktop 中 `Icon::new(IconName::X)` 与 `Button::icon(...)` 按 gpui-component 的图标名解析到上述资源 ID。

## 依赖关系
- 依赖的外部模块：无（纯静态 SVG 文件）。
- 被谁依赖：ui-desktop 模块（`Assets` 实现与界面图标）。

## 常见改动点
- 替换/新增图标文件时，保持文件名与 `Assets::load` 中的 ID 一致。
- 新增图标需同时修改 ui-desktop 的 `Assets::load` 分支；必要时同步 design-prototype 的 Font Awesome 图标。
- 批量换风格时按全部 47 个 SVG 一起处理，注意线性风格与品牌色约束。

## 已知风险 / 易错点
- 47 个文件中只有 22 个在 `Assets::load` 登记；`book-open`、`search`、`close`、`plus` 虽已登记但当前代码未使用。
- 修改文件名或路径而未同步 `include_bytes!` 会导致编译失败。
- 每个图标通过独立 `include_bytes!` 嵌入，增删时需核对分支列表完整性。
