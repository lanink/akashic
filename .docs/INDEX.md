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

## 项目概述
Akashic（阿卡西）是“相册整理师”桌面应用：定位本地隐私优先的批量照片整理工具，PRD 规划智能扫描、相似归组、模糊检测、截图/大文件筛选、批量删除与回收站、智能重命名、智能归档等能力。M1 已接入真实本地索引（目录选择/后台扫描/EXIF/内容哈希/SQLite/缩略图/目录过滤/搜索）。M2 已实现分析与智能整理：模糊评分、截图启发式、大文件排序、相似归组（pHash + dssim + 并查集），扫描后自动后台分析，整理页 4 个工具卡片接入真实结果列表。M3 已实现批量操作与回收站：批量删除（移动到回收站，绝不永久丢失）、批量重命名（预设模板+预览+冲突处理+一次撤销）、回收站恢复/清空/30 天清理，操作日志贯穿。智能归档仍属后续里程碑（占位）。

## 目录职责表

| 目录 / 文件 | 职责 | 对应模块 md |
| --- | --- | --- |
| `src/main.rs` | 组合根：Application/Assets/GPUI 初始化、本地目录初始化、打开窗口 | [ui-desktop](modules/ui-desktop.md) |
| `src/lib.rs` | 库入口：`pub mod core; pub mod ui;` | [build-config](modules/build-config.md) |
| `src/core/model.rs` / `config.rs` | 真实数据模型（Photo/Folder/ScanStats）与本地路径配置 | [core-model](modules/core-model.md) |
| `src/core/db.rs` / `scanner.rs` / `thumbnail.rs` | SQLite 索引、递归扫描+EXIF+哈希、缩略图缓存、分析结果读写 | [core-scanner](modules/core-scanner.md) |
| `src/core/analyze/*.rs` | 模糊评分、截图识别、大文件排序、相似归组（pHash+dssim+并查集） | [core-analyze](modules/core-analyze.md) |
| `src/core/ops/*.rs` | 回收站（删除/恢复/清空/30 天清理/撤销）与批量重命名模板引擎、操作日志 | [core-ops](modules/core-ops.md) |
| `src/ui/app.rs` / `assets.rs` | gpui 渲染：全局状态、页面分发、目录树、照片弹窗、整理页结果列表、重命名页、回收站页、图标资源 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/library.rs` | UI 数据适配层：索引数据 → 相册库视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/analyze.rs` | UI 分析适配层：分析结果 → 整理页视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/ops.rs` | UI 批量操作适配层：回收站/操作日志 → 回收站页视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `tests/scanner_test.rs` | 扫描/增量/缩略图集成测试（临时目录，不碰真实相册） | [core-scanner](modules/core-scanner.md) |
| `tests/analyze_test.rs` | 分析链路集成测试（清晰/模糊/重复/截图样本） | [core-analyze](modules/core-analyze.md) |
| `tests/ops_test.rs` | 批量操作集成测试（重命名100张+撤销、删除恢复、冲突、清空、30天清理） | [core-ops](modules/core-ops.md) |
| `vendor/imageflow/` | imageflow git submodule（固定 commit `7cbe26be`），图像编解码 | [build-config](modules/build-config.md) |
| `.gitmodules` | submodule 配置：vendor/imageflow → GitHub 地址 | [build-config](modules/build-config.md) |
| `akashic-ds.html` | 单文件 HTML 交互设计稿，5 个页面 + 照片详情弹窗 | [design-prototype](modules/design-prototype.md) |
| `design.md` | 产品需求文档（定位、MVP 功能、信息架构、视觉基调、交付物状态） | [product-spec](modules/product-spec.md) |
| `readme.md` | 项目名与空版本说明，待补充 | [product-spec](modules/product-spec.md) |
| `Cargo.toml` / `Cargo.lock` | 包定义、依赖与版本锁定 | [build-config](modules/build-config.md) |
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
                    ├──> core-model（数据模型 + 路径配置）
                    ├──> core-scanner（SQLite 索引 + 扫描 + 缩略图）
                    ├──> core-analyze（M2 分析与智能整理）
                    │            │ 依赖
                    │            v
                    │      core-scanner（db 读写分析结果）
                    └──> core-ops（M3 回收站 + 批量重命名 + 操作日志）
                                 │ 依赖
                                 v
                           core-scanner（db 读写回收站/日志）+ core-model + core-config
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
- 页面分发：`src/ui/app.rs::AppRoot::render_page`（Page::Home / Organize / Library / Rename / Trash）。
- 扫描入口：`AppState::start_scan` → 后台 `core::scanner::scan_root` → `core::db` 写库 → channel 回传进度 → `LibraryData::reload`。
- 分析入口：扫描完成后自动 `AppState::start_analysis`（或整理页「重新分析」）→ 后台 `core::analyze::analyze_library` → `core::db` 写回分析结果 → `AnalyzeProgress` 回传 → `AnalyzeData::reload`。
- 批量删除入口：相册库「删除到回收站」→ `AppState::batch_trash` → `core::ops::trash::trash_photos`（移动到回收站+写库）→ `reload_all`；撤销 `undo_trash`。
- 重命名入口：相册库「重命名」→ `Page::Rename`（预设模板+预览）→ `AppState::batch_rename` → `core::ops::rename::execute`；撤销 `undo_rename`。
- 回收站入口：侧栏「回收站」→ `Page::Trash`（`OpsData`）→ 恢复 `restore_items` / 清空 `empty_trash` / 30 天清理 `purge_expired`。
- 真实数据：`src/core/db.rs`（Repository）、`src/core/scanner.rs`（扫描）、`src/ui/library.rs`（视图适配）；模型在 `core/model.rs`，路径在 `core/config.rs`。
- 分析算法：`src/core/analyze/`（blur / classify / similarity / pipeline）；测试 `tests/analyze_test.rs`。
- 批量操作：`src/core/ops/`（trash / rename / mod）；测试 `tests/ops_test.rs`。
- 设计稿入口：`akashic-ds.html` 的 `#page-library`（默认 active）与 `switchPage(pageId)`。
- 构建验证：`cargo check --manifest-path D:/Code/Freespace/akashic/Cargo.toml`（2026-11-16 通过，exit 0）。

## 文档状态总览

| 模块 | 覆盖 hash | 状态 | 可信度 |
| --- | --- | --- | --- |
| ui-desktop | d6aab21a0a19 | valid | AI 生成未审 |
| core-model | e7729140f3e4 | valid | AI 生成未审 |
| core-scanner | c56f5c922fb5 | valid | AI 生成未审 |
| core-analyze | f49408cd46d5 | valid | AI 生成未审 |
| core-ops | 77b5e8c922de | valid | AI 生成未审 |
| design-prototype | d32b2f6af661 | valid | AI 生成未审 |
| product-spec | 2df241d5d3bf | valid | AI 生成未审 |
| build-config | 532872a43d8b | valid | AI 生成未审 |
| icon-assets | 4f60f3a7bd02 | valid | AI 生成未审 |
| ai-workflow | 3dfb897e056b | valid | AI 生成未审 |

> 初始化提示：以上目录职责与依赖图由 AI 生成，需人类审阅确认；后续结构性变更须同步更新本文件。
