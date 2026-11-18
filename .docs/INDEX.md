# Akashic 项目顶层索引

> 生成时间：2026-11-16 16:08:19 +0800
> 说明：按 `.docs/WORKFLOW.md` 第 9 节初始化生成；请人类审阅目录职责与依赖图。
> 约定：源码是唯一真相；md 为派生缓存，冲突时以源码为准并刷新对应 md。
> 覆盖 hash 算法：对覆盖文件按相对路径排序，逐行写 `相对路径  sha256sum(内容)`，再取整体 sha256 前 12 位。
> 更新日期：2026-11-16 16:15 +0800（新增开发计划入口）。
> 更新日期：2026-11-16 17:35 +0800（M0 分层：src 拆为 core/ 与 ui/；新增 core-model 模块 md；imageflow 改 submodule）。
> 更新日期：2026-11-18 15:10 +0800（M1 真实索引：新增 core-scanner 模块 md；相册库/首页接真实数据；新增 M1 依赖）。

## 项目概述
Akashic（阿卡西）是“相册整理师”桌面应用：定位本地隐私优先的批量照片整理工具，PRD 规划智能扫描、相似归组、模糊检测、截图/大文件筛选、批量删除与回收站、智能重命名、智能归档等能力。M1 已接入真实本地索引：原生目录选择、后台递归扫描、EXIF 元数据、内容哈希、SQLite 索引、缩略图缓存、目录过滤与搜索；相册库/首页显示真实数据。相似归组、模糊检测等整理算法与批量操作属后续里程碑，页面仍为占位。

## 目录职责表

| 目录 / 文件 | 职责 | 对应模块 md |
| --- | --- | --- |
| `src/main.rs` | 组合根：Application/Assets/GPUI 初始化、本地目录初始化、打开窗口 | [ui-desktop](modules/ui-desktop.md) |
| `src/lib.rs` | 库入口：`pub mod core; pub mod ui;` | [build-config](modules/build-config.md) |
| `src/core/model.rs` / `config.rs` | 真实数据模型（Photo/Folder/ScanStats）与本地路径配置 | [core-model](modules/core-model.md) |
| `src/core/db.rs` / `scanner.rs` / `thumbnail.rs` | SQLite 索引、递归扫描+EXIF+哈希、缩略图缓存 | [core-scanner](modules/core-scanner.md) |
| `src/ui/app.rs` / `assets.rs` | gpui 渲染：全局状态、页面分发、目录树、照片弹窗、图标资源 | [ui-desktop](modules/ui-desktop.md) |
| `src/ui/library.rs` | UI 数据适配层：索引数据 → 相册库视图模型 | [ui-desktop](modules/ui-desktop.md) |
| `tests/scanner_test.rs` | 扫描/增量/缩略图集成测试（临时目录，不碰真实相册） | [core-scanner](modules/core-scanner.md) |
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
                    v
              core-model（数据模型 + 路径配置）
                    ^
                    │ 使用
              core-scanner（SQLite 索引 + 扫描 + 缩略图）
                    │
                    v
              build-config（Cargo.toml / Cargo.lock / .gitmodules）
                    │
                    ├─> rusqlite(bundled) / kamadak-exif / walkdir / image / twox-hash / chrono（crates.io）
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
- 真实数据：`src/core/db.rs`（Repository）、`src/core/scanner.rs`（扫描）、`src/ui/library.rs`（视图适配）；模型在 `core/model.rs`，路径在 `core/config.rs`。
- 设计稿入口：`akashic-ds.html` 的 `#page-library`（默认 active）与 `switchPage(pageId)`。
- 构建验证：`cargo check --manifest-path D:/Code/Freespace/akashic/Cargo.toml`（2026-11-16 通过，exit 0）。

## 文档状态总览

| 模块 | 覆盖 hash | 状态 | 可信度 |
| --- | --- | --- | --- |
| ui-desktop | a777486cc90b | valid | AI 生成未审 |
| core-model | 26c49c6040ee | valid | AI 生成未审 |
| core-scanner | 808d37dcd3f9 | valid | AI 生成未审 |
| design-prototype | d32b2f6af661 | valid | AI 生成未审 |
| product-spec | 2df241d5d3bf | valid | AI 生成未审 |
| build-config | 532872a43d8b | valid | AI 生成未审 |
| icon-assets | 4f60f3a7bd02 | valid | AI 生成未审 |
| ai-workflow | 3dfb897e056b | valid | AI 生成未审 |

> 初始化提示：以上目录职责与依赖图由 AI 生成，需人类审阅确认；后续结构性变更须同步更新本文件。
