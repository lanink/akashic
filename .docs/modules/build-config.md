# 模块：build-config

> 生成时间：2026-11-16 17:35 +0800
> 覆盖范围：.gitmodules, Cargo.lock, Cargo.toml（按路径排序）
> 覆盖 hash：999b41b033fb（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
定义 Rust 包 akashic 的构建方式、依赖版本、产物入口（lib + bin），以及 imageflow git submodule 的引入配置。

## 对外接口
- 包：`akashic` version `0.1.0`，edition `2024`。
- `[lib]`：`name = "akashic"`，`path = "src/lib.rs"`（内容为 `pub mod core; pub mod ui;`）。
- `[[bin]]`：`name = "akashic"`，`path = "src/main.rs"`（组合根）。
- crates.io 依赖：`dssim = "3.4.0"`、`gpui = "0.2.2"`、`gpui-component = "0.5.1"`、`image = "0.25.10"`、`anyhow = "1.0.104"`、`dirs = "4.0.0"`。
- submodule 路径依赖：`imageflow_core`、`imageflow_types`，路径为 `vendor/imageflow/imageflow_core` 与 `vendor/imageflow/imageflow_types`。
- `.gitmodules`：`vendor/imageflow` → `https://github.com/imazen/imageflow.git`，固定 commit `7cbe26be`。

## 内部关键流程
1. 换机构建先执行 `git submodule update --init`（从 GitHub 拉取 imageflow 到 `vendor/imageflow` 并 checkout 到固定 commit）。
2. `cargo check` / `cargo build` 解析 Cargo.lock → 拉取 crates.io 依赖 + 编译 submodule 内的 imageflow 本地 crate。
3. 编译 `src/lib.rs` 生成库目标，编译 `src/main.rs` 生成同名二进制。
4. 当前验证结果：2026-11-16 17:20 左右 `cargo check` 与 `cargo test` 均 exit 0；test 通过 1 项（`core::config` 路径派生）。仅有 `proc-macro-error2 v2.0.1` 未来版本兼容性提示。

## 依赖关系
- 依赖的外部模块：crates.io 依赖 + `vendor/imageflow` submodule 内的 imageflow crate（c-codecs 默认 feature，需 NASM）。
- 被谁依赖：ui-desktop 与 core-model 模块的编译都依赖此配置。

## 常见改动点
- 增减功能依赖时同时更新 Cargo.toml 与 Cargo.lock，并刷新本模块 hash。
- 升级 imageflow 固定 commit：在 submodule 内 checkout 新 commit → 提交父仓库的 gitlink → 验证编译。
- 切换 zen-codecs：网络可访问 GitHub 时，按 Cargo.toml 注释启用 feature 并加 `[patch.crates-io]`（zenjpeg/zenwebp）。
- 切换 gpui / gpui-component 大版本时需复核 ui-desktop 的 API 用法。

## 已知风险 / 易错点
- `zen-codecs`（目标配置）依赖 zenjpeg/zenwebp 的 git main 分支，本机 GitHub 网络 502 无法拉取，当前基线暂用 c-codecs（本机有 NASM）；切换方法见 Cargo.toml 注释与 DEV_PLAN。
- `dirs` 必须锁定 4.0.0（与 Cargo.lock 一致），改 6.x 会触发离线解析失败。
- `dssim`、`image`、`imageflow_*` 当前源码未直接调用，为预置依赖；功能尚待 M1/M2 落地。
- Windows 上多次后台 cargo 会因文件锁死锁，出现"Blocking waiting for file lock"时先清理残留 cargo 进程再单进程编译。

## 变更记录
- 2026-11-16 17:35：M0 结构性变更，imageflow 由仓库外 `../imageflow/` 改为固定 commit 的 `vendor/imageflow` submodule；新增 `dirs` 依赖；覆盖范围加入 `.gitmodules`，整模块重写。
