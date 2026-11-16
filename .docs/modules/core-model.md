# 模块：core-model

> 生成时间：2026-11-16 17:35 +0800
> 覆盖范围：src/core/config.rs, src/core/mod.rs, src/core/model.rs（按路径排序）
> 覆盖 hash：e28c0dec42be（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的纯数据模型与本地路径配置，不依赖 gpui，可独立单元测试。M0 阶段沿用设计稿模拟数据，M1 将用真实索引数据库替换。

## 对外接口
- `pub struct PhotoDef`（src/core/model.rs）：模拟照片定义，字段 `name/date/size/file_size/device/location/tags/hue` 均 `pub`。
- `pub const PHOTOS: &[PhotoDef]`：12 张模拟照片。
- `pub struct FolderNode`（src/core/model.rs）：目录树节点，字段 `name/count/children` 均 `pub`。
- `pub fn folder_defs() -> Vec<FolderNode>`：构造静态相册目录树（日本旅行/生日派对/四姑娘山/2024年/2023年及子目录）。
- `pub fn expandable_id(folders: &[FolderNode], target: usize) -> Option<usize>`：深度优先扁平索引到可展开节点下标的映射。
- `pub const APP_ID: &str = "akashic"`（src/core/config.rs）：应用标识。
- `pub struct Paths`（src/core/config.rs）：本地路径集合，字段 `data_dir/cache_dir/trash_dir/db_path/log_path` 均 `pub`。
- `pub fn Paths::from_app_dir(base: PathBuf) -> Self`：基于给定基目录派生全部路径。
- `pub fn Paths::platform_default() -> Option<Self>`：用 `dirs::data_local_dir()` 定位平台应用数据目录。
- `pub fn Paths::ensure_dirs(&self) -> std::io::Result<()>`：幂等创建全部目录。

## 内部关键流程
1. UI 层通过 `folder_defs()` / `PHOTOS` 读取模拟数据渲染相册库与目录树。
2. 目录展开状态由 `expandable_id` 将深度优先扁平索引映射到 `AppState.folder_expanded` 数组下标。
3. 组合根 `main.rs` 启动时调用 `Paths::platform_default()` → `ensure_dirs()`，确保索引库、缩略图缓存、回收站目录存在。
4. 单元测试 `paths_derive_from_base` 在临时基目录验证路径派生正确性（无需真实文件系统写入）。

## 依赖关系
- 依赖的外部模块：`dirs`（crate，定位应用数据目录）、`std::fs`/`std::path`。
- 被谁依赖：`ui-desktop`（读取 `PHOTOS`/`folder_defs`/`expandable_id`）、`main.rs` 组合根（读取 `Paths`）。

## 常见改动点
- M1 接入真实索引：替换 `PHOTOS`/`folder_defs` 为数据库查询，保持对外接口形状或同步更新 UI 调用方。
- 新增配置项：在 `Paths` 增加字段并在 `from_app_dir` 派生，同步补 `paths_derive_from_base` 断言。
- 新增纯数据模型（`Photo`/`Folder`/`Operation` 等）：在本模块新增文件并在 `core/mod.rs` 注册 pub mod。

## 已知风险 / 易错点
- `dirs` 版本必须与 Cargo.lock 一致（当前 4.0.0），写 6.x 会触发联网解析导致离线构建失败。
- `Paths::platform_default()` 在极少数无标准应用数据目录的平台返回 `None`，调用方需处理。
- 本模块不得引入 gpui 依赖，否则破坏 `main → ui → core` 单向分层与可测试性。

## 变更记录
- 2026-11-16 17:35：M0 结构性变更，新建 core 层；从原 `main.rs` 迁入纯数据模型，新增 `config.rs` 本地路径配置。
