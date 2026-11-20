# 模块：build-config

> 生成时间：2026-11-20 16:35 +0800
> 覆盖范围：.gitignore, .gitmodules, Cargo.lock, Cargo.toml, scripts/package_portable.ps1（按路径排序）
> 覆盖 hash：2ad507641c53（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
定义 Rust 包 akashic 的构建方式、依赖版本、产物入口（lib + bin），以及 imageflow git submodule 配置；M6 追加 `[profile.release]` 优化配置、便携版打包脚本与 `dist/` 忽略规则。

## 对外接口
- 包：`akashic` version `0.1.0`，edition `2024`。
- `[lib]`：`name = "akashic"`，`path = "src/lib.rs"`（`pub mod core; pub mod ui;`）。
- `[[bin]]`：`name = "akashic"`，`path = "src/main.rs"`。
- crates.io 依赖：
  - UI：`gpui = "0.2.2"`、`gpui-component = "0.5.1"`、`rfd = "0.15.4"`。
  - 核心：`rusqlite = { version = "0.35.0", features = ["bundled"] }`、`kamadak-exif = "0.6.2"`、`walkdir = "2.5.0"`、`image = "0.25.10"`、`dssim = "3.4.0"`、`twox-hash = "2.1.3"`、`chrono = "0.4.45"`、`dirs = "4.0.0"`。
  - 通用：`anyhow = "1.0.104"`。
- submodule 路径依赖：`imageflow_core`、`imageflow_types` → `vendor/imageflow/*`。
- `.gitmodules`：`vendor/imageflow` → `https://github.com/imazen/imageflow.git`，固定 commit `7cbe26be`。
- `[profile.release]`（M6）：`opt-level = 3`、`lto = "thin"`、`codegen-units = 1`、`strip = true`；不设 `panic = "abort"`，保证 `cargo test --release` 可用。
- 打包脚本 `scripts/package_portable.ps1`（M6）：`cargo build --release` → 收集 exe / readme / CHANGELOG → 写 `LICENSE-NOTICE.txt` → 压缩生成 `dist/akashic-0.1.0-win-x86_64.zip`。
- `.gitignore`：排除 `/target`、`/dist/`（便携包二进制不入库）、`/.workbuddy/`、IDE 目录。

## 内部关键流程
1. 换机构建：`git submodule update --init` 拉取 imageflow → `cargo check/build` 解析 Cargo.lock → 编译 crates.io 依赖 + submodule 内 imageflow。
2. `rusqlite bundled` 内嵌 SQLite C 源码，经 `libsqlite3-sys v0.33.0` 编译（M1 已在本机 MSVC 工具链验证通过）。
3. 产物：`target/debug/akashic.exe`（dev，约 54MB）与 `target/release/akashic.exe`（M6 release profile，strip 后更小）。
4. 验证结果：2026-11-19 `cargo check` / `cargo test` / `cargo build` 均通过；41 个常规测试 + 1 个 `#[ignore]` 性能基准。
5. M6 便携包：`scripts/package_portable.ps1` 生成 `dist/akashic-0.1.0-win-x86_64.zip`（exe + 文档 + 许可说明），`dist/` 已加入 `.gitignore`。

## 依赖关系
- 依赖的外部模块：crates.io 依赖 + `vendor/imageflow` submodule（c-codecs，需 NASM）。
- 被谁依赖：ui-desktop 与 core 各模块的编译均依赖此配置。

## 常见改动点
- 增减依赖：同时更新 Cargo.toml 与 Cargo.lock，并刷新本模块 hash。
- 版本锁定：`dirs` 必须 4.0.0（离线 lock 一致）；新依赖优先选已在 lock 中的版本以避免联网。
- 切换 imageflow commit：submodule checkout 新 commit → 提交父仓库 gitlink → 验证编译。
- 切换 zen-codecs：网络可访问 GitHub 时按 Cargo.toml 注释启用 feature 并加 `[patch.crates-io]`。
- 发布构建与打包：改 `[profile.release]` 需重跑 `cargo build --release` 与打包脚本；版本号变化时同步脚本内 `$Version` 与 zip 名。

## 已知风险 / 易错点
- `zen-codecs`（目标）依赖 zenjpeg/zenwebp 的 git main 分支，本机 GitHub 502 拉不到，当前用 c-codecs（有 NASM）；切换方法见 Cargo.toml 注释与 DEV_PLAN。
- `rusqlite bundled` 首次编译较慢（C 源码），且依赖 `cc`/MSVC 工具链。
- Windows 多次后台/中断 cargo 会因文件锁死锁；出现「Blocking waiting for file lock」时清理残留 cargo 进程后单进程编译。
- `image 0.25` 与 `imageflow` 各有解码路径，本期缩略图用 image，避免重复解码同一张原图。

- `[profile.release]` 用 `lto="thin"` + `codegen-units=1`，首次 release 编译明显慢于 dev；打包脚本依赖 NASM 与已初始化的 submodule。

## 变更记录
- 2026-11-19：M6 新增 `[profile.release]` 优化配置、`scripts/package_portable.ps1` 便携包脚本，`.gitignore` 增加 `/dist/`；覆盖范围纳入 `.gitignore` 与打包脚本并刷新 hash。
- 2026-11-18 15:10：M1 新增 rusqlite/kamadak-exif/walkdir/chrono/rfd/twox-hash 依赖，整模块重写。
- 2026-11-16 17:35：M0 imageflow 改 submodule，新增 dirs。
