# Akashic（阿卡西纪）· 相册整理师

本地优先的桌面照片整理工具：不修图、不社交、不上传，把杂乱的相册扫描进本机索引，
再按相似 / 模糊 / 截图 / 大文件筛选，批量删除（可恢复）、重命名、归档（可撤销）。

> 状态：M0–M5 已完成（工程地基 / 真实索引 / 分析整理 / 批量操作与回收站 / 智能归档 / 首页与体验闭环）。
> 全部功能离线可用，照片与元数据只保存在本机应用数据目录。

## 1. 环境要求

| 项 | 要求 |
| --- | --- |
| Rust | 1.85+（项目使用 edition 2024；本机验证工具链 1.98） |
| C 工具链 | `rusqlite bundled` 需要 C 编译器；Windows 建议 MSVC Build Tools |
| NASM | imageflow 当前基线使用 `c-codecs`，Windows 上需要 NASM（已加入 PATH） |
| 系统 | Windows / macOS / Linux（gpui 0.2.2 桌面窗口） |
| 网络 | 首次 `cargo build` 与 `git submodule update` 需要网络；**运行期完全不需要** |

> 目标照片不含 iPhone HEIC/HEIF；本期不做 HEIC 解码，也不做照片地图足迹（规划保留、暂缓实施）。

## 2. 获取代码与子模块

imageflow 以固定 commit `7cbe26be` 的 git submodule 放在 `vendor/imageflow/`：

```bash
git clone <仓库地址> akashic
cd akashic
git submodule update --init --recursive
```

若 clone 时已带 `--recurse-submodules`，可跳过最后一步。子模块缺失时 `cargo build` 会因
`path = "vendor/imageflow/imageflow_core"` 解析失败，这是最常见的换机构建错误。

## 3. 构建、运行与测试

```bash
cargo check                      # 类型检查
cargo test                       # 全部单元 + 集成测试（仅使用临时目录，不碰真实相册）
cargo build                      # 产物 target/debug/akashic.exe（Windows 约 54MB）
cargo run                        # 启动窗口
```

测试分层：

- `tests/scanner_test.rs`：临时目录扫描、增量更新、缩略图。
- `tests/analyze_test.rs`：模糊 / 清晰 / 重复 / 截图样本的分析链路。
- `tests/ops_test.rs`：重命名 100 张 + 一次撤销、删除恢复、冲突、清空、30 天清理。
- `tests/archive_test.rs`：归档目录树与落盘一致、复制保留源文件、撤销恢复原路径、4 种规则与兜底。
- `tests/settings_test.rs`：回收站保留期设置的默认值、往返、脏值回退与越界校验。

## 4. 第一次使用

1. 启动后默认进入「相册库」，点右上角 **选择目录** 选中照片根目录。
2. 后台递归扫描（EXIF 拍摄时间 / 设备 / 内容哈希 / 400px 缩略图缓存），进度显示在相册库状态条。
3. 扫描完成自动触发分析：相似归组、模糊评分、截图识别、大文件排序。
4. 到「智能整理」查看 4 类结果；「重命名」「智能归档」卡片分别进入对应页面。
5. 批量操作遵循「筛选 → 预览 → 确认（可撤销）」：删除先进回收站，归档先展示完整目录树预览。
6. 「回收站」可恢复、撤销上次删除、按保留期清理或清空；「设置与隐私」可查看本机数据路径、修改保留期。

多选：**Ctrl + 点按**（Windows 上 Win 键即 `platform` 修饰键也可）。普通点击照片打开原图大图与完整信息。

## 5. 数据放在哪里

全部路径由 `src/core/config.rs::Paths` 从平台应用数据目录派生：

| 平台 | 根目录 |
| --- | --- |
| Windows | `%LOCALAPPDATA%\akashic` |
| macOS | `~/Library/Application Support/akashic` |
| Linux | `~/.local/share/akashic` |

目录下：`index.sqlite3`（SQLite 索引 / 分析结果 / 操作日志 / settings，WAL 模式）、
`cache/thumbnails/`（缩略图）、`trash/`（回收站文件）、`akashic.log`。
程序不会在照片目录里写任何东西，也不会把照片上传到任何服务。

## 6. 项目结构与文档导航

```text
src/main.rs            组合根（Application / Assets / 窗口）
src/lib.rs             pub mod core; pub mod ui;
src/core/              不依赖 gpui，可独立单测：model / db / scanner / thumbnail / analyze / ops
src/ui/                gpui 渲染：app.rs（状态与页面）/ assets.rs / library.rs / analyze.rs / ops.rs
tests/                 集成测试（临时目录）
vendor/imageflow/       固定 commit 的 git submodule
.docs/                 文档体系
```

- 项目规则：`CODEBUDDY.md`（Claude Code 用户看 `Claude.md`）→ **每次任务先读 `.docs/WORKFLOW.md`**。
- 文档导航：`.docs/INDEX.md`（目录职责 / 依赖图 / 关键入口 / 文档状态总览）→ `.docs/modules/*.md`。
- 开发计划与里程碑交付记录：`.docs/DEV_PLAN.md`；PRD 见 `design.md`；交互设计稿见 `akashic-ds.html`。
- 源码是唯一真相，`.docs/modules/` 下的 md 是派生缓存；改动接口/结构后需按 WORKFLOW 刷新覆盖 hash。

依赖方向必须是 `main.rs → ui → core → 外部设施`，`core` 不得引用 `gpui`。

## 7. 已知限制

- 本期不做 HEIC/HEIF、RAW 解码；不支持格式由缩略图缓存兜底显示。
- 照片足迹 / 地图展示（OxiGeo、GCJ-02、腾讯地图 GL JS）为暂缓项，规划保留在 DEV_PLAN 的 M4+。
- imageflow 目前用 `c-codecs`（需 NASM）；`zen-codecs` 纯 Rust 路径依赖 GitHub 上的 zenjpeg/zenwebp git main，
  切换配置已写在 `Cargo.toml` 注释与 DEV_PLAN 风险表中，网络可访问后再启用。
- 批量删除 / 重命名 / 归档为同步执行（个人库规模够用）；5000 张以上的性能剖析与虚拟滚动属 M6。
- 「故事」归档规则按「年 / 年-月_地点或首标签」推导，不是人物或事件聚类。
- 复制模式归档、回收站恢复产生的副本索引不继承分析结果，需要重新分析。
- imageflow 为 AGPLv3；本软件定位个人本地使用，若未来商业分发需重新评估授权。
