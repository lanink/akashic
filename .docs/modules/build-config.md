# 模块：build-config

> 生成时间：2026-11-16 16:08:19 +0800
> 覆盖范围：Cargo.toml, Cargo.lock（按路径排序）
> 覆盖 hash：0e9624ece6f0（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
定义 Rust 包 akashic 的构建方式、依赖版本与产物入口（lib + bin），并锁定依赖解析结果。

## 对外接口
- 包：`akashic` version `0.1.0`，edition `2024`。
- `[lib]`：`name = "akashic"`，`path = "src/lib.rs"`。
- `[[bin]]`：`name = "akashic"`，`path = "src/main.rs"`。
- 依赖：`dssim = "3.4.0"`、`gpui = "0.2.2"`、`gpui-component = "0.5.1"`、`image = "0.25.10"`、`anyhow = "1.0.104"`。
- 本地路径依赖：`imageflow_core`、`imageflow_helpers`、`imageflow_types`，路径分别为 `../imageflow/imageflow_core`、`../imageflow/imageflow_helpers`、`../imageflow/imageflow_types`。

## 内部关键流程
1. `cargo check` / `cargo build` 解析 Cargo.lock → 拉取 crates.io 依赖并链接同级目录的 imageflow 本地 crate。
2. 编译 `src/lib.rs` 生成库目标，编译 `src/main.rs` 生成同名二进制。
3. 当前验证结果：`cargo check --manifest-path D:/Code/Freespace/akashic/Cargo.toml` 于 2026-11-16 16:08 左右完成，exit 0；仅有 `proc-macro-error2 v2.0.1` 未来版本兼容性提示。

## 依赖关系
- 依赖的外部模块：crates.io 依赖与仓库外的 `../imageflow/*` 本地 crate。
- 被谁依赖：ui-desktop 模块的所有编译与运行都依赖此配置。

## 常见改动点
- 增减功能依赖时同时更新 Cargo.toml 与 Cargo.lock。
- 切换 gpui / gpui-component 大版本时需复核 ui-desktop 的 API 用法。
- 启用相似性、模糊检测等算法能力时，移除未使用依赖或补上实际调用。

## 已知风险 / 易错点
- 三个 imageflow 依赖位于仓库外的 `../imageflow/`，单独克隆或移动本项目会导致构建失败。
- `dssim`、`image`、`imageflow_*` 在当前源码中没有调用，仅为预置依赖；Cargo 不会因此报错，但功能尚未落地。
- 项目 Git 仓库尚无提交，所有源码与配置文件均处于未跟踪状态（2026-11-16 检查）。
