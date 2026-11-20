# Akashic 项目顶层索引

> 生成时间：2026-11-16 16:08:19 +0800
> 说明：按 `.docs/WORKFLOW.md` 第 9 节初始化生成；请人类审阅目录职责与依赖图。
> 约定：源码是唯一真相；md 为派生缓存，冲突时以源码为准并刷新对应 md。
> 覆盖 hash 算法：对覆盖文件按相对路径排序，逐行写 `相对路径  sha256sum(内容)`，再取整体 sha256 前 12 位。
> 更新日期：2026-11-16 16:15 +0800（新增开发计划入口）。
> 更新日期：2026-11-16 17:35 +0800（M0 分层：src 拆为 core/ 与 ui/；新增 core-model 模块 md；imageflow 改 submodule）。
> 更新日期：2026-11-18 15:10 +0800（M1 真实索引：新增 core-scanner 模块 md；相册库/首页接真实数据；新增 M1 依赖）。
> 更新日期：2026-11-18 15:40 +0800（M2 分析与智能整理：新增 core-analyze 模块 md 与 ui/analyze.rs、tests/analyze_test.rs；整理页接真实分析结果与后台分析调度）。
> 更新日期：2026-11-18 16:05 +0800（M3 批量操作与回收站：新增 core-ops 模块 md 与 ui/ops.rs、tests/ops_test.rs；相册库批量删除/重命名、重命名页与回收站页）。
> 更新日期：2026-11-18 16:35 +0800（修复缩略图不显示：所有照片展示位接入 gpui `img(PathBuf)` 真实缩略图，详情弹窗升级为原图大图 + 完整图像信息；刷新 ui-desktop hash）。
> 更新日期：2026-11-18 16:58 +0800（M4 智能归档：新增 core/ops/archive.rs 与归档页，4 种规则 / 移动·复制 / 完整目录树预览 / 撤销；整理页卡片与侧栏徽标改真实；刷新 4 个模块 hash）。
> 更新日期：2026-11-18 17:40 +0800（M5 首页与体验闭环：首页真实最近操作/整理进度/加载·空·错误态、设置与隐私页、可配置回收站保留期、窗口自适应与最小尺寸、图标 47/47 全登记、清理 M0 模拟数据、readme 重写；刷新 5 个模块 hash 与状态总览）。
> 更新日期：2026-11-19 08:55 +0800（M6 打磨与交付：相册库 uniform_list 虚拟滚动、5000 张性能基准、[profile.release]、便携版打包脚本与产物、design.md 状态刷新、CHANGELOG 与 readme 发布章节；全量刷新模块 hash 与状态总览）。
> 更新日期：2026-11-19 10:38 +0800（全量 bug 审查与修复：数据安全 / 索引正确性 / 分析准确性 / 批量重命名 / 智能归档 / 缩略图与 UI 交互；新增 22 个回归测试，63 个常规测试全绿；刷新 6 个模块 hash 与状态总览，ui-desktop 覆盖范围纳入 src/main.rs）。
> 更新日期：2026-11-19 11:33 +0800（修复相册库网格不显示：gpui `div()` 默认 `Display::Block` 导致 content/page-library 高度链断裂、`uniform_list` 得不到确定高度，`grid-info` 竖排塌缩；显式 `.flex().flex_col()` + `flex_1` 打通高度链并给列表 `.size_full()`；刷新 ui-desktop hash）。
> 更新日期：2026-11-19 11:40 +0800（补修网格宽度：Block 容器内列表百分比宽度回退成 min-content，照片被压成竖线；`photo-grid-scroll` 改 `.flex().flex_col()` + 行 `.w_full()` + tile `min_w(120)` 兜底；刷新 ui-desktop hash）。
> 更新日期：2026-11-20 16:30 +0800（修复相册库缩略图不显示：根因是 tile 用 `Button`，其内部布局让 `img().size_full()` 塌陷；改为与整理页一致的普通 `div` + 明确尺寸缩略图容器；刷新 ui-desktop hash）。

## 项目概述
Akashic（阿卡西）是“相册整理师”桌面应用：定位本地隐私优先的批量照片整理工具，PRD 规划智能扫描、相似归组、模糊检测、截图/大文件筛选、批量删除与回收站、智能重命名、智能归档等能力。M1 已接入真实本地索引（目录选择/后台扫描/EXIF/内容哈希/SQLite/缩略图/目录过滤/搜索）。M2 已实现分析与智能整理：模糊评分、截图启发式、大文件排序、相似归组（pHash + dssim + 代表元聚类），扫描后自动后台分析，整理页 4 个工具卡片接入真实结果列表。M3 已实现批量操作与回收站：批量删除（移动到回收站，绝不永久丢失）、批量重命名（预设模板+预览+冲突处理+一次撤销）、回收站恢复/清空/可配置保留期清理，操作日志贯穿。所有照片展示位（相册库网格/整理结果/相似组/回收站）已接入 gpui `img(PathBuf)` 真实缩略图，点击照片弹出原图大图 + 完整图像信息（基本信息/分析结果/索引字段）。M4 已实现智能归档：按年/年月/故事/标签 4 种规则生成完整目录树预览，支持移动与复制两种模式，执行后可一次撤销；整理页「智能重命名/智能归档」卡片与侧栏「智能整理」徽标均已接真实数据。M5 完成体验闭环：首页统计/整理进度/最近操作全部接真实数据并补加载·空·错误态，新增设置与隐私中心（本机路径、可配置回收站保留期、数据概况、隐私说明、关于），窗口去掉 1200×780 强绑定并设最小尺寸，图标注册补齐 47/47，清理 M0 模拟数据，readme.md 重写为可独立上手的文档。M6 完成打磨与交付：相册库改为 gpui `uniform_list` 虚拟滚动（每行 4 张、等高，只渲染视口内行），新增 5000 张照片性能基准与 `[profile.release]` 优化配置，提供 Windows 便携版打包脚本与 zip 产物，刷新 `design.md` 交付物状态与实现差异、新增 `CHANGELOG.md`、readme 补发布章节，并全量复核模块 hash。2026-11-19 完成一轮全量 bug 审查与修复（数据安全、索引正确性、分析准确性、批量重命名、智能归档、缩略图缓存与 UI 交互），新增 22 个回归测试，`cargo test` 63 个常规测试全绿。地图足迹仍属本期暂缓。

## 目录职责表

| 目录 / 文件 | 职责 | 对应模块 md |
| --- | --- | --- |
| `src/main.rs` | 组合根：Application/Assets/GPUI 初始化、本地目录初始化、窗口初始尺寸与 `window_min_size`、启动失败显式报错 | [ui-desktop](modules/ui-desktop.md) |
| `src/lib.rs` | 库入口：`pub mod core; pub mod ui;` | [build-config](modules/build-config.md) |
| `src/core/model.rs` / `config.rs` | 真实数据模型（Photo/Folder/ScanStats）、共享 `BLUR_THRESHOLD` 与本地路径配置 | [core-model](modules/core-model.md) |
| `src/core/db.rs` / `scanner.rs` / `thumbnail.rs` | SQLite 索引、递归扫描+EXIF+哈希、缩略图缓存、分析结果/操作日志/settings 读写 | [core-scanner](modules/core-scanner.md) |
| `src/core/analyze/*.rs` | 模糊评分、截图识别、大文件排序、相似归组（pHash+dssim+代表元聚类） | [core-analyze](modules/core-analyze.md) |
| `src/core/ops/*.rs` | 回收站（删除/恢复/清空/保留期清理/撤销）、批量重命名模板引擎、M4 智能归档（4 规则 / 移动·复制 / 目录树预览 / 执行 / 撤销）、操作日志 | [core-ops](modules/core-ops.md) |
| `src/ui/app.rs` / `assets.rs` | gpui 渲染：全局状态、页面分发（7 页含智能归档/设置与隐私）、目录树、M6 `uniform_list` 虚拟滚动网格、照片图片渲染（缩略图/原图大图）、详情弹窗、整理页结果列表、重命名页、归档页、回收站页、首页真实统计与状态态、图标资源（47 个 SVG 全登记） | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/library.rs` | UI 数据适配层：索引数据 → 相册库视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/analyze.rs` | UI 分析适配层：分析结果 → 整理页视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/ops.rs` | UI 批量操作适配层：回收站/操作日志 → 回收站页视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `tests/scanner_test.rs` | 扫描/增量/缩略图/LIKE 转义/sweep 集成测试（临时目录，不碰真实相册） | [core-scanner](modules/core-scanner.md) |
| `tests/analyze_test.rs` | 分析链路集成测试（清晰/模糊/重复/截图/坏图/pHash 聚类样本） | [core-analyze](modules/core-analyze.md) |
| `tests/ops_test.rs` | 批量操作集成测试（重命名100张+撤销、删除恢复、冲突、清空、30天清理、无序号模板、注入防护、id 去重、保留期越界） | [core-ops](modules/core-ops.md) |
| `tests/archive_test.rs` | M4 归档集成测试（目录树与落盘一致、复制保留源文件、撤销恢复原路径、冲突不覆盖、4 规则与缺失元数据兜底、预览只读/settings、目标校验与副本不增殖） | [core-ops](modules/core-ops.md) |
| `tests/settings_test.rs` | M5 设置项集成测试（保留期默认值/往返/脏值回退/越界拒绝） | [core-ops](modules/core-ops.md) |
| `tests/perf_test.rs` | M6 性能基准（5000 张合成图：扫描/冷启动/筛选耗时，`#[ignore]`） | [core-scanner](modules/core-scanner.md) |
| `scripts/package_portable.ps1` | M6 Windows 便携版打包脚本（release 构建 → zip，含许可说明） | [build-config](modules/build-config.md) |
| `dist/` | M6 便携版打包产物（zip，已被 `.gitignore` 忽略，不入库） | 无 |
| `CHANGELOG.md` | 0.1.0 版本变更记录（M0–M6 新增/修复/已知限制，含 2026-11-19 全量修复条目） | [product-spec](modules/product-spec.md) |
| `.gitignore` | 构建产物 `/target`、便携包 `/dist/`、`/.workbuddy/` 等忽略规则 | [build-config](modules/build-config.md) |
| `vendor/imageflow/` | imageflow git submodule（固定 commit `7cbe26be`），图像编解码 | [build-config](modules/build-config.md) |
| `.gitmodules` | submodule 配置：vendor/imageflow → GitHub 地址 | [build-config](modules/build-config.md) |
| `akashic-ds.html` | 单文件 HTML 交互设计稿，5 个页面 + 照片详情弹窗 | [design-prototype](modules/design-prototype.md) |
| `design.md` | v1 PRD（定位、MVP 功能、信息架构、视觉基调）；M6 刷新头部状态、第 5 节交付物真实状态并新增第 8 节实现差异 | [product-spec](modules/product-spec.md) |
| `readme.md` | M5 重写、M6 扩为 8 节：环境、submodule、构建/运行/测试、首次使用、数据目录、文档导航、发布与便携包、已知限制 | [product-spec](modules/product-spec.md) |
| `Cargo.toml` / `Cargo.lock` | 包定义、依赖与版本锁定、M6 `[profile.release]` | [build-config](modules/build-config.md) |
| `assets/icons/*.svg` | 47 个 Lucide 线性图标，编译期嵌入 | [icon-assets](modules/icon-assets.md) |
| `.docs/WORKFLOW.md` | AI 工作协议（Agent 不得修改） | [ai-workflow](modules/ai-workflow.md) |
| `.docs/DEV_PLAN.md` | 开发计划（规划文档，非派生缓存，不参与覆盖 hash） | 无 |
| `Claude.md` / `CODEBUDDY.md` | Claude Code 与 WorkBuddy/CodeBuddy 项目级规则入口 | [ai-workflow](modules/ai-workflow.md) |
| `.docs/INDEX.md` / `.docs/modules/*.md` | 本索引与派生模块文档 | 由 WORKFLOW.md 规定格式 |
| `target/` | Cargo 构建产物，不作为源码真相 | 无 |
| `.git/` / `.idea/` / `.meituan-catpaw/` | 版本库与本地工具目录 | 无 |

## 模块依赖图

```text
product-spec（PRD / 视觉规范）
        │ 参考
        ├──> design-prototype（HTML 设计稿）
        │            │ 参照布局与配色
        │            v
        └──> main.rs（组合根）
                    │ 调用
                    v
              ui-desktop（src/ui/，gpui 渲染）───> icon-assets（SVG 图标）
                    │ 依赖（单向）              ───> rfd（目录对话框）
                    ├──> core-model（数据模型 + 路径配置 + BLUR_THRESHOLD）
                    ├──> core-scanner（SQLite 索引 + 扫描 + 缩略图）
                    ├──> core-analyze（M2 分析与智能整理）
                    │            │ 依赖
                    │            v
                    │      core-scanner（db 读写分析结果）
                    └──> core-ops（M3 回收站 + 批量重命名 + M4 智能归档 + 操作日志）
                                 │ 依赖
                                 v
                           core-scanner（db 读写回收站/日志/settings）+ core-model + core-config
                    │
                    v
              build-config（Cargo.toml / Cargo.lock / .gitmodules）
                    │
                    ├─> rusqlite(bundled) / kamadak-exif / walkdir / image / twox-hash / chrono / dssim（crates.io）
                    └─> vendor/imageflow（git submodule，固定 7cbe26be，c-codecs）

约束：main → ui → core → 外部设施，core 不得引用 gpui。
ai-workflow ──> 约束所有 Agent 对以上模块的读取与文档更新
```

## 关键入口

- 产品规则：`CODEBUDDY.md` / `Claude.md` → `.docs/WORKFLOW.md`（每次任务先读）。
- 文档导航：`.docs/INDEX.md`（本文件）→ `.docs/modules/<模块>.md`。
- 程序入口：`src/main.rs::main()` → `Paths::platform_default/ensure_dirs` → `ui::app::new_app_state`（加载索引）→ `AppRoot::new` → `Root::new`。
- 页面分发：`src/ui/app.rs::AppRoot::render_page`（Page::Home / Organize / Library / Rename / Archive / Trash / Settings）。
- 扫描入口：`AppState::start_scan` → 后台 `core::scanner::scan_root` → `core::db` 写库 → channel 回传进度 → `reload_all`（每轮排空 channel）。
- 分析入口：扫描完成后自动 `AppState::start_analysis`（或整理页「重新分析」；忙时 `pending_analysis` 排队）→ 后台 `core::analyze::analyze_library` → `core::db` 写回分析结果 → `AnalyzeProgress` 回传 → `reload_all`。
- 批量删除入口：相册库「删除到回收站」→ `AppState::batch_trash` → `core::ops::trash::trash_photos`（移动到回收站+写库）→ `reload_all`；撤销 `undo_trash`。
- 重命名入口：相册库「重命名」→ `Page::Rename`（预设模板 + 前缀输入框 + 预览）→ `AppState::batch_rename` → `core::ops::rename::execute`；撤销 `undo_rename`。
- 回收站入口：侧栏「回收站」→ `Page::Trash`（`OpsData`）→ 恢复 `restore_items` / 清空 `empty_trash` / 按设置保留期清理 `purge_expired`。
- 归档入口：侧栏「智能归档」或整理页「智能归档」卡片 → `Page::Archive` → `AppState::refresh_archive_plan` → `core::ops::archive::plan`（只读目录树预览，校验目标绝对路径且不在照片目录内）→ `execute_archive` → `core::ops::archive::execute`（Move/Copy）→ 写 `ArchiveMove`/`ArchiveCopy` 日志；撤销 `undo_archive` → `archive::undo`。规则/模式/目标目录持久化在 `settings`（`archive.rule/mode/dest`）。
- 图片渲染入口：`src/ui/app.rs::photo_thumb(&Photo)` → `thumbnail_source`（缩略图缓存优先，源文件不存在返回 None）→ `gpui::img(PathBuf)`（`Resource::Path` → `fs::read` + `image` 解码，不经过 `Assets`）→ `ObjectFit::Cover` + fallback/loading 色块降级。详情大图走 `preview_source`（原图优先）+ `ObjectFit::Contain`。
- 图片详情入口：普通点击照片 → `open_photo_dialog(photo: Photo, window, cx)` → 原图大图 + 基本信息/分析结果/索引分区 + 「打开原图」/「所在文件夹」（按文件存在性禁用）。
- 首页入口：`Page::Home` → `page_home`：统计卡来自 `LibraryData`，整理进度来自 `AnalyzeData`/`analyze_status`，最近整理记录来自 `OpsData.recent_ops`（`OpKind::label()`）；错误态由全局横幅统一展示，首页 banner 只保留进度态。
- 设置与隐私入口：侧栏「设置」→ `Page::Settings` → `page_settings`：本机 `Paths` 路径、保留期按钮 `ops_trash::set_retention_days`、真实数据概况、隐私说明、关于；回收站页「清理 N 天前」跟随 `retention_days`。
- 窗口尺寸：`src/main.rs` 的 `WindowOptions.window_min_size = 960×640`，`AppRoot::render` 的 `windowcard` 用 `.size_full()` 跟随窗口。
- 分析算法：`src/core/analyze/`（blur / classify / similarity / pipeline）；测试 `tests/analyze_test.rs`。
- 批量操作：`src/core/ops/`（trash / rename / archive / mod）；测试 `tests/ops_test.rs` / `tests/archive_test.rs`。
- 设计稿入口：`akashic-ds.html` 的 `#page-library`（默认 active）与 `switchPage(pageId)`。
- 虚拟滚动入口：`src/ui/app.rs::page_library` → `gpui::uniform_list("photo-virtual-grid", row_count, f)`，按 `GRID_COLUMNS=4` / `GRID_ROW_HEIGHT=162px` 等高 lazy 渲染。
- 性能基准入口：`cargo test --release --test perf_test -- --ignored --nocapture`（`tests/perf_test.rs`，5000 张合成图）。
- 便携包入口：`scripts/package_portable.ps1` → `dist/akashic-0.1.0-win-x86_64.zip`（`cargo build --release` + 文档 + 许可说明）。
- 构建验证：`cargo check` / `cargo test` / `cargo build`（2026-11-19 复核：63 个常规测试全绿 + 1 个 `#[ignore]` 基准，debug 构建通过；M6 release 便携包为本轮之前产物，未重新打包）。

## 文档状态总览

| 模块 | 覆盖 hash | 状态 | 可信度 |
| --- | --- | --- | --- |
| ui-desktop | de03a9b5b1a7 | valid | AI 生成未审 |
| core-model | f8ad5300fb64 | valid | AI 生成未审 |
| core-scanner | 0e97b65fe29f | valid | AI 生成未审 |
| core-analyze | a9809cab5871 | valid | AI 生成未审 |
| core-ops | f76508cc0b9d | valid | AI 生成未审 |
| design-prototype | d32b2f6af661 | valid | AI 生成未审 |
| product-spec | b20e6ab38201 | valid | AI 生成未审 |
| build-config | 44dcb0c47faf | valid | AI 生成未审 |
| icon-assets | 4f60f3a7bd02 | valid | AI 生成未审 |
| ai-workflow | 3dfb897e056b | valid | AI 生成未审 |

> 初始化提示：以上目录职责与依赖图由 AI 生成，需人类审阅确认；后续结构性变更须同步更新本文件。
