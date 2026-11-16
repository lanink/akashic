# Akashic 项目顶层索引

> 生成时间：2026-11-16 16:08:19 +0800
> 说明：按 `.docs/WORKFLOW.md` 第 9 节初始化生成；请人类审阅目录职责与依赖图。
> 约定：源码是唯一真相；md 为派生缓存，冲突时以源码为准并刷新对应 md。
> 覆盖 hash 算法：对覆盖文件按相对路径排序，逐行写 `相对路径  sha256sum(内容)`，再取整体 sha256 前 12 位。
> 更新日期：2026-11-16 16:15 +0800（新增开发计划入口）。

## 项目概述
Akashic（阿卡西）是“相册整理师”桌面应用的产品原型：定位本地隐私优先的批量照片整理工具，PRD 规划了智能扫描、相似归组、模糊检测、截图/大文件筛选、批量删除与回收站、智能重命名、智能归档等能力。当前代码只实现了 gpui 界面原型（首页、智能整理入口、相册库、占位页、照片详情弹窗），数据全部为静态模拟，未接入真实文件系统扫描或图像处理算法。

## 目录职责表

| 目录 / 文件 | 职责 | 对应模块 md |
| --- | --- | --- |
| `src/main.rs` | gpui 应用入口、全局状态、页面渲染、目录树、照片弹窗、资源加载 | [ui-desktop](modules/ui-desktop.md) |
| `src/lib.rs` | 模板示例库函数与单元测试，未被二进制引用 | [ui-desktop](modules/ui-desktop.md) |
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
        └──> ui-desktop（Rust / gpui UI）───> icon-assets（SVG 图标）
                    │
                    v
              build-config（Cargo.toml / Cargo.lock）
                    │
                    v
        ../imageflow/*（仓库外本地 crate，构建时依赖）

ai-workflow ──> 约束所有 Agent 对以上模块的读取与文档更新
```

## 关键入口

- 产品规则：`CODEBUDDY.md` / `Claude.md` → `.docs/WORKFLOW.md`（每次任务先读）。
- 文档导航：`.docs/INDEX.md`（本文件）→ `.docs/modules/<模块>.md`。
- 程序入口：`src/main.rs::main()` → `AppState::new` → `AppRoot` → `Root::new`。
- 页面分发：`AppRoot::render_page`（Page::Home / Organize / Library / Rename / Trash）。
- 设计稿入口：`akashic-ds.html` 的 `#page-library`（默认 active）与 `switchPage(pageId)`。
- 构建验证：`cargo check --manifest-path D:/Code/Freespace/akashic/Cargo.toml`（2026-11-16 通过，exit 0）。

## 文档状态总览

| 模块 | 覆盖 hash | 状态 | 可信度 |
| --- | --- | --- | --- |
| ui-desktop | 3de2319b1dca | valid | AI 生成未审 |
| design-prototype | d32b2f6af661 | valid | AI 生成未审 |
| product-spec | 2df241d5d3bf | valid | AI 生成未审 |
| build-config | 0e9624ece6f0 | valid | AI 生成未审 |
| icon-assets | 4f60f3a7bd02 | valid | AI 生成未审 |
| ai-workflow | 3dfb897e056b | valid | AI 生成未审 |

> 初始化提示：以上目录职责与依赖图由 AI 生成，需人类审阅确认；后续结构性变更须同步更新本文件。
