# Akashic 开发计划

> 编制时间：2026-11-16 16:15 +0800  
> 更新时间：2026-11-16 16:32 +0800（确认个人使用 / imageflow 子模块 / 无 HEIC，新增照片足迹）  
> 更新时间：2026-11-16 16:44 +0800（照片足迹相关功能本期暂缓，保留规划不实施；开始 M0 工程地基）  
> 更新时间：2026-11-18 15:10 +0800（M0 完成；M1 真实索引完成：目录选择/递归扫描/EXIF/内容哈希/SQLite/缩略图/目录过滤/搜索，相册库与首页接真实数据）
> 更新时间：2026-11-18 15:40 +0800（M2 完成：模糊评分/截图识别/大文件排序/相似归组，后台分析调度，整理页 4 工具接真实结果列表）  
> 文档性质：规划文档，非 `.docs/modules/` 下的派生缓存；不参与覆盖 hash 校验。  
> 依据：`design.md`（PRD v1）与 2026-11-16 的源码现状（见 `.docs/INDEX.md`）。

## 1. 目标与边界

把当前 gpui 界面原型实现为**本地优先、可放心批量整理照片的桌面工具**。MVP 交付 PRD 第 3 节的 7 项能力：智能扫描、相似归组、模糊检测、截图/大文件筛选、批量删除+回收站、智能重命名、智能归档。

范围外（本期不做）：修图、社交分享、云同步、AI 人物识别、移动端 App、HEIC/HEIF 解码。

**本期暂缓（规划保留、不实施）**：照片足迹 / 地图展示（原 M4+ 及 OxiGeo、GCJ-02、腾讯地图 GL JS 相关依赖）。2026-11-16 决定先聚焦整理能力，地图功能待整理链路闭环后再评估启动。相关章节与依赖表保留为未来入口，标注“暂缓”以免被误当作本期任务。

已确认前提（2026-11-16）：

1. 软件定位为**个人本地使用**，不做商业分发；imageflow 的 AGPLv3 许可证可接受。
2. imageflow 以**固定 commit 的 git submodule** 引入，固定 `7cbe26be`。
3. 目标照片**不含 iPhone HEIC**，本期不引入 HEIC 解码依赖。
4. 桌面优先，沿用现有 gpui 1200×780 窗口与 5 项侧栏导航；PRD 中的“4 个底部 Tab”在桌面端映射为侧栏入口。
5. 纯本地运行，索引、缩略图、操作日志、照片坐标都存放在本机应用数据目录，不上传任何照片或位置数据。
6. 所有批量操作遵循 PRD 的“筛选 → 预览 → 确认（可撤销）”，删除一律先进入回收站，不做永久删除。
7. 性能目标按目标用户“5000+ 张照片”设定，扫描与分类必须在后台线程执行，UI 不阻塞。

## 2. 现状差距

| 领域    | 当前状态                                             | 目标状态                  |
| ----- | ------------------------------------------------ | --------------------- |
| UI 外壳 | 标题栏/侧栏/5 页面/相册库网格/照片详情弹窗已可运行                     | 保留视觉，替换为真实数据与真实操作     |
| 数据    | `PHOTOS`、`folder_defs()`、首页统计全部硬编码               | 本地索引数据库 + 文件系统真实元数据   |
| 扫描    | 无目录选择、无遍历、无增量更新                                  | 目录选择、递归扫描、增量索引、缩略图缓存  |
| 算法    | `dssim`/`image`/`imageflow_*` 仅声明未调用             | 相似归组、模糊评分、截图/大文件筛选    |
| 操作    | 工具卡片、详情弹窗按钮、重命名/回收站页均为占位                         | 删除/恢复/撤销、模板重命名、规则归档   |
| 工程    | Git 无提交；`src/lib.rs` 是模板 `add()`；代码集中在 `main.rs` | 首次提交；核心/UI 分层；可测试、可维护 |

## 3. 目标架构

```text
src/
  main.rs                 仅做组合根：Application、Assets、GPUI 初始化、打开窗口
  lib.rs                  真正的库入口：pub mod core; pub mod ui;
  core/                   不依赖 gpui，可独立单元测试
    model.rs              Photo / Folder / PhotoGroup / Operation / Settings
    db.rs                 SQLite schema、迁移、Repository
    scanner.rs            目录遍历、元数据提取、内容哈希、增量更新、缩略图
    analyze/
      similarity.rs       缩略图 dssim + pHash 预筛 + 并查集分组
      blur.rs             灰度缩放后 Laplacian 方差评分
      classify.rs         截图启发式、大文件排序、标签推导
    geo/                  【本期暂缓】照片足迹相关，暂不创建该目录
      coordinates.rs      EXIF GPS 提取结果 WGS84 → GCJ-02 转换（暂缓）
      footprints.rs       足迹聚合、GeoJSON 导出（按需启用 OxiGeo，暂缓）
    ops/
      journal.rs           操作日志与 undo/redo
      trash.rs             移入回收站、恢复、30 天清理
      rename.rs            模板解析、预览、冲突处理、撤销
      archive.rs           归档规则、目录树预览、移动/复制、撤销
  ui/                     只依赖 core，不直接碰文件系统
    app.rs                AppState / AppRoot / 页面分发（从 main.rs 迁出）
    pages/                home / organize / library / rename / trash / settings（footprint 本期暂缓）
    components/           sidebar / folder_tree / photo_grid / photo_dialog / preview_bar
    assets.rs             Assets 实现与图标 ID 注册表
tests/
  fixtures/               模糊、清晰、重复、截图样本
  scanner_test.rs         临时目录扫描与增量测试
  ops_test.rs             删除/恢复/重命名/归档的端到端测试
```

依赖方向必须是 `main.rs → ui → core → 外部设施`，`core` 不得引用 `gpui`。这样才能在不开窗口的情况下测试业务规则。

## 4. 技术选型与依赖职责（2026-11-16 讨论结论）

已核对本地 `../imageflow` 仓库（HEAD `7cbe26be`）与 GitHub README。`imageflow_core` 是执行引擎（解码、约束/重采样、裁剪、旋转、EXIF 方向、色彩处理、编码），`imageflow_types` 定义作业图的 `Node` / `Execute001` / `EncoderPreset`，`imageflow_helpers` 是内部工具集。README 明确说明其 Rust API **不稳定、不与 C ABI 版本 semver 对齐**；仓库默认 `c-codecs` 依赖 NASM/Visual Studio Build Tools，`zen-codecs` 是纯 Rust 编解码路径。许可证为 AGPLv3，商业分发需评估授权或购买例外。

| 依赖                             | 状态                   | 在项目中的作用                                                                                                    | 关键约束                                                                                                                        |
| ------------------------------ | -------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `gpui = 0.2.2`                 | 保留                   | 桌面窗口、事件循环、Element 渲染                                                                                       | 版本较旧，UI 代码集中在 `ui/`，避免扩散                                                                                                    |
| `gpui-component = 0.5.1`       | 保留                   | `Root`、`Button`、`Dialog`、`Icon` 等现成组件                                                                      | 与 gpui 版本强耦合，升级需整体回归                                                                                                        |
| `anyhow`                       | 保留                   | 应用统一错误边界，把 imageflow/SQLite/IO 错误转成用户可读信息                                                                  | 核心库对外返回 `anyhow::Result`                                                                                                    |
| `imageflow_core`               | 保留但改配置               | 缩略图流水线：解码 → 应用 EXIF 方向 → 约束/缩放 → 编码到缓存；也负责格式转换                                                             | 使用 `default-features = false, features = ["zen-codecs"]`，避开 C 工具链；固定本地 commit                                               |
| `imageflow_types`              | 保留                   | 构造作业图：`Node::Decode`、`Node::ApplyOrientation`、`Node::Constrain` / `Resample2D`、`Node::Encode`、`Execute001` | 必须显式依赖，不能只靠 `imageflow_core` 的传递依赖                                                                                          |
| `imageflow_helpers`            | 移除直接依赖               | 不再直接用；其 hashing/filesystem 工具属于 imageflow 内部实现                                                             | 保留在 imageflow_core 的传递依赖中即可，避免绑定不稳定 API                                                                                     |
| `image = 0.25.10`              | 保留                   | 像素级分析：灰度化、缩放、Laplacian 模糊评分、给 dssim 提供缓冲区                                                                  | 不再作为主缩略图流水线；避免与 imageflow 重复解码                                                                                              |
| `dssim = 3.4.0`                | 保留                   | 相似照片精排；先用 pHash 粗筛缩小候选集                                                                                    | 输入用缩略图而非原图                                                                                                                  |
| `kamadak-exif`（或 `exif`）       | 新增                   | 读取拍摄时间、设备、GPS、Orientation 标签                                                                               | 与 imageflow 的 `ApplyOrientation` 配合，方向只应用一次                                                                                 |
| `rusqlite`（`bundled`）          | 新增                   | 本地照片索引、分组结果、操作日志、设置                                                                                        | 单文件本地库，事务保证批量操作可撤销                                                                                                          |
| `serde` / `serde_json`         | 新增                   | 配置、重命名模板、操作记录、imageflow 作业参数序列化                                                                            | imageflow 类型本身已用 serde，版本需兼容                                                                                                |
| `walkdir`                      | 新增                   | 递归扫描目录，配合增量索引                                                                                              | 过滤隐藏目录与回收站目录                                                                                                                |
| `rfd`                          | 新增                   | 原生文件夹选择对话框                                                                                                 | 仅在用户主动触发时调用                                                                                                                 |
| `xxhash_rust`                  | 新增                   | 快速内容哈希，作为缩略图缓存 key 与重复检测辅助                                                                                 | 与文件大小、尺寸联合判断，避免单独依赖哈希                                                                                                       |
| `tracing` + `tracing-appender` | 新增                   | 扫描/分类/批量操作日志，可导出诊断                                                                                         | 默认写应用数据目录，不写照片目录                                                                                                            |
| `dirs`（或 `directories`）        | 新增                   | 定位应用数据目录、缓存目录、回收站目录                                                                                        | 不同平台路径统一由 core 提供                                                                                                           |
| `tempfile`                     | 新增（dev）              | 测试中为扫描/删除/重命名/归档提供临时目录                                                                                     | 禁止用真实相册做破坏性测试                                                                                                               |
| `rayon`                        | 按需新增                 | 批量分析与缩略图生成并行化                                                                                              | imageflow 的 `zen-codecs` 已带 rayon，先复用再决定是否直依赖                                                                               |
| `oxigeo`                       | **暂缓**（原按需）          | 照片足迹数据的 GeoJSON 导入/导出、EPSG CRS 变换、可选空间聚类                                                                   | 本期不引入；未来重启足迹功能时再评估，只开需要的 feature：`default-features = false, features = ["geojson"]`（需要投影时再加 `proj`）；MSRV 1.95，当前工具链 1.98 满足 |
| GCJ-02 转换                      | **暂缓**（原自实现或小 crate） | EXIF 的 WGS84 坐标转为国内地图要求的 GCJ-02                                                                            | 本期不实现；未来重启时只做坐标偏移，不用带 OSM 边界 mask 的库做领土判断；个人本地数据仅本地保存                                                                       |
| 腾讯位置服务 GL JS                   | **暂缓**（原运行时依赖）       | 渲染照片足迹地图与标记                                                                                                | 本期不引入；未来重启时独立桌面应用需自行申请 key，代码中只写占位符，禁止内置可用 key                                                                              |

明确不做：`imageflow_abi` / `imageflow_tool` / `imageflow_server`（直接调用 Rust API）、OCR、云同步、HEIC/HEIF 解码、Google/Apple/Bing/Mapbox/OSM 瓦片与第三方未资质地图源。

> **2026-11-16 M0 实测修正（zen-codecs 并非纯 crates.io）**：原判断“`zen-codecs` 无 C 依赖、可避开 NASM”只对一半。查固定 commit 的 `imageflow/Cargo.toml` 发现，`zen-codecs` 依赖 `zenjpeg`、`zenwebp` 两个 **git main 分支**（`CmykHandling` 等 API 未发布到 crates.io），还带 `libpng-sys` / `lcms2-sys` 的 `[patch.crates-io]`。作为独立项目引用 submodule 时这些 patch 不会继承，会解析到 crates.io 旧版（zenjpeg 0.8.4）导致 `CmykHandling` 编译错误；必须复制 `[patch.crates-io]` 才能用 git 版（0.8.7）。本机网络对 GitHub 报 502，拉不到这两个 git 依赖，故 **M0 基线暂用默认的 `c-codecs`**（本机已验证有 NASM，可编译），并在 Cargo.toml 注释中写好 zen-codecs 切换配置。待网络可访问 GitHub 后再切换并重验。这是环境阻塞，不是方案否定；`zen-codecs` 仍为既定目标。

使用边界：**imageflow 只负责“把图像变成目标像素/文件”，不负责分类决策**；**OxiGeo 只负责地理数据读写与坐标运算，不负责渲染地图、不提供底图瓦片**。模糊、相似、截图、大文件的判断放在 `core/analyze/`，地图展示交给合规地图服务。

OxiGeo 可行性结论：**可以做，但不是照片足迹的核心依赖**。它能提供 GeoJSON 导出、EPSG 坐标变换、空间聚类与 MBTiles/PMTiles 读写；不能提供底图、不能在 gpui 里画地图、不做 GCJ-02 偏移、不做逆地理编码。最小可行方案是：`kamadak-exif` 读 GPS → 自实现 WGS84→GCJ-02 → 生成带腾讯地图 GL JS 的 HTML → 在 WebView 或系统浏览器中显示标记。只有当需要“导出足迹 GeoJSON / 按地理位置聚类故事 / 离线瓦片缓存”时才引入 OxiGeo，并只开 `geojson`（必要时 `proj`）feature，避免把 65+ crate 的生态全拉进来。

风险与应对：AGPLv3 在个人使用下可接受；Rust API 不稳定所以固定 commit `7cbe26be` 并隔离在 `core/imaging.rs`；imageflow 用 git submodule 引入；地图合规要求使用腾讯/高德/百度/天地图等有资质服务，坐标用 GCJ-02，位置数据只存本地。新增/调整依赖后必须同步更新 `.docs/modules/build-config.md` 的覆盖 hash 与 `.docs/INDEX.md` 的依赖图。

## 5. 核心数据模型

`Photo`：id、path、file_name、size_bytes、exif_date、modified_at、width、height、device、location、gps_lon_wgs84、gps_lat_wgs84、gps_lon_gcj02、gps_lat_gcj02、gps_source、tags、content_hash、blur_score、is_screenshot、group_id、favorite、trash_state、archived_path。

> 其中 6 个 gps\_* 字段与地图展示相关，本期暂缓阶段 schema 可预留列但不采集、不转换、不展示，避免为暂缓功能付出实现成本。

`Folder`：id、parent_id、path、label、photo_count，由索引路径派生，不再手写 `folder_defs()`。

`PhotoGroup`：id、rule、representative_photo_id、member_ids、created_at。

`Operation`：id、batch_id、kind（rename/archive/delete/restore）、before、after、created_at、undone_at。

`Settings`：scan_roots、trash_retention_days（默认 30）、rename_template、archive_rule、thumbnail_size。

## 6. 里程碑

### M0 · 工程地基

任务：Git 首次提交建立基线；移除 `src/lib.rs` 的 `add()` 模板；按第 3 节拆分 `core/` 与 `ui/` 骨架，`main.rs` 瘦身为组合根；接入配置路径与日志；建立 `cargo test` 基线。

验收：`cargo check` 与 `cargo test` 通过；窗口视觉与当前一致；`core` 中不出现 gpui 引用。此阶段属于**结构性变更**，须重写 ui-desktop 模块文档、新增 core/ui 子模块文档并更新 INDEX 依赖图。

### M1 · 真实索引与相册库 — 已完成（2026-11-18）

任务：实现目录选择、`walkdir` 递归扫描、EXIF 解析、内容哈希、缩略图生成与增量更新；建立 SQLite schema 与 Repository；用数据库替换 `PHOTOS` 和 `folder_defs()`；目录树从真实路径派生；目录点击过滤照片网格；基础排序与搜索。

验收：选择一个含 N 张照片的目录，相册库显示 N 张真实照片；重复扫描不产生重复记录；5000 张照片的扫描和滚动不阻塞 UI。完成后重写 ui-desktop 模块文档并新增 scanner/db 模块文档。

> **实际交付（2026-11-18）**：目录选择（rfd）、walkdir 扫描、EXIF（时间/设备）、twox-hash 内容哈希、`image` 缩略图缓存（400px）、SQLite schema + Repository、增量 upsert 与 sweep 清理、目录树派生与点击过滤、关键词搜索均已实现；相册库与首页统计改为真实数据。8 个测试通过（含临时目录扫描集成测试）。**偏差与遗留**：① `rusqlite bundled` 依赖已验证可编译，crates.io 可访问（GitHub git 依赖仍不通，zen-codecs 未切换）；② 缩略图 UI 仍用色块占位，`thumbnail_for` 已返回缓存路径，接 gpui `img()` 即可显示真图；③ 排序 UI 未做（Repository 已按时间倒序）；④ 5000 张性能基线未实测；⑤ M0 模拟数据常量待清理。已同步 ui-desktop/core-model/core-scanner/build-config 模块 md 与 INDEX。

### M2 · 分析与智能整理 — 已完成（2026-11-18）

任务：实现相似归组、模糊评分、截图识别、大文件筛选；结果落库；后台任务带进度；`page_organize` 的 6 个工具卡片分别进入真实列表页；组内预览与“保留最佳”交互。

验收：fixture 测试能稳定识别模糊/重复/截图样本；工具卡片、列表页、预览页形成闭环；ui-desktop 模块文档的工具卡片风险项被消除。

> **实际交付（2026-11-18）**：新增 `core/analyze/`（blur Laplacian 方差评分、classify 截图启发式+大文件排序、similarity pHash+DSSIM+并查集、pipeline 编排与进度）；db 增 `photo_groups` 表与 4 个分析列（含老库迁移）；扫描完成自动触发后台分析或整理页手动重分析；整理页前 4 工具卡片 badge 改真实值并可点击进入相似组/模糊/截图/大文件结果列表。24 个测试通过（20 lib + 2 analyze 集成 + 2 scanner 集成）。新增 `core-analyze` 模块 md 与 `ui/analyze.rs` 适配层，刷新 core-model/core-scanner/ui-desktop/INDEX。**偏差与遗留**：① 结果列表缩略图仍用色块占位（缓存路径已就绪，待 M5 接 `img()`）；② 后 2 个工具（重命名/归档）徽标标为 M3/M4，未进入列表页（属对应里程碑）；③ “保留最佳”/删除交互属 M3 回收站范围，本阶段仅展示结果列表；④ dssim 精排为尽力而为（坏图回退纯 pHash）；⑤ 5000 张分析性能未实测（pHash O(n²) + dssim 候选组内）。已同步 core-analyze/core-model/core-scanner/ui-desktop 模块 md 与 INDEX。

### M3 · 批量操作与回收站

任务：实现回收站目录与 30 天清理策略；批量删除前展示数量与占用空间；恢复、清空；重命名模板引擎（前缀/日期/时间/标签/地点/序号）、3-5 张预览、冲突处理、撤销；操作日志贯穿所有批量操作。

验收：可批量重命名 100 张并一次撤销；删除的文件可在回收站恢复；任何路径下都不会永久删除。完成后新增 ops 子模块文档并更新 ui-desktop 的按钮行为描述。

### M4 · 智能归档

任务：按年/月/故事/标签归档（人物识别留作后续）；执行前展示完整目录树预览；移动/复制可选；归档后支持撤销。

验收：规则生成的目录树与实际落盘一致；复制模式不删除源文件；撤销恢复原路径。

### M4+ · 照片足迹（地图）— 本期暂缓，不实施

任务：`kamadak-exif` 提取 GPS → WGS84→GCJ-02 转换 → 本地 SQLite 保存坐标；新增足迹页，按时间/地点聚合照片；生成带腾讯地图 GL JS 的本地 HTML（地图标记用 `TMap.MultiMarker`，坐标为 GCJ-02），通过系统浏览器打开，后续再评估 WebView 内嵌。需要导出 GeoJSON 或按地理聚类故事时，才启用 OxiGeo 的 `geojson`（必要时 `proj`）feature。

验收：有 GPS 的照片能在地图上按位置显示标记，点击标记回到对应照片；无 GPS 的照片不参与；坐标数据只存在本机，不上传；地图 key 只以占位符出现在代码/文档中，不内置可用 key。

说明：独立桌面应用不属于 WorkBuddy 的代理免 key 场景，需自行到腾讯位置服务开放平台申请 key，配置 Referer 白名单并把占位符 `key="Please apply for your own key at Tencent Location Service Open Platform and replace this placeholder"` 替换为自己的 key；不得使用 Google/Apple/Bing/Mapbox/OSM 等未合规地图源。

### M5 · 首页与体验闭环

任务：首页统计、整理进度、最近操作改为真实数据；补齐加载态、空态、错误态；设置/隐私中心页面；清理 `Assets::load` 中未使用或缺失的图标注册；去掉硬编码 1200×780 的强绑定，设置合理最小尺寸。

验收：全应用不再出现模拟数字；离线状态下所有功能可用；readme.md 可让新人独立跑起来。

### M6 · 打磨与交付

任务：性能剖析与虚拟滚动网格；便携版/安装包；更新 readme.md、design.md 状态与设计交付物清单；生成 CHANGELOG；全量刷新模块 md 的 hash 与 INDEX 状态总览。

验收：5000 张照片场景下冷启动、扫描、筛选耗时可接受；文档 hash 校验全部为 valid。

## 7. 测试与验收原则

- 单元：模板解析、模糊阈值、分组算法、哈希冲突处理。
- 集成：临时目录中的扫描→索引→筛选→删除→恢复→撤销完整链路。
- UI 冒烟：每个里程碑结束后按“筛选 → 预览 → 确认 → 撤销”逐页手测一遍。


- 性能基线：5000 张照片索引完成时间、相似分组完成时间、网格滚动帧率，逐里程碑记录。
- 破坏性操作测试只能在临时目录进行，禁止用真实相册做验证。

## 8. 风险与应对

| 风险                               | 影响                                                                | 应对                                                                                     |
| -------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `../imageflow/*` 在仓库外            | 换机器直接构建失败                                                         | M0 改为 git submodule 或 vendored，并固定 commit `7cbe26be`                                   |
| imageflow Rust API 不稳定           | 升级 imageflow 可能破坏编译                                               | 只在 `core/imaging.rs` 适配；依赖版本不跟随 C ABI tag                                              |
| imageflow `zen-codecs` 依赖 git 分支 | 需访问 GitHub 拉 zenjpeg/zenwebp main，且需复制 `[patch.crates-io]`；本机 502 | M0 基线暂用 c-codecs（本机有 NASM 可编译）；Cargo.toml 已注释切换配置，网络恢复后切换并重验；长期目标仍是 zen-codecs 免 C 工具链 |
| imageflow AGPLv3                 | 商业分发可能需要授权                                                        | 已确认个人本地使用，暂不处理；若未来分发再评估授权或例外                                                           |
| 地图源合规                            | 使用未资质地图源会有合规风险                                                    | 只用腾讯/高德/百度/天地图；默认腾讯地图；坐标使用 GCJ-02                                                      |
| 地图 key 泄露或过期                     | 足迹地图不可用                                                           | 自行申请 key，代码中只保留占位符，配置 Referer 白名单，禁止内置可用 key                                           |
| OxiGeo 过度引入                      | 编译体积与维护成本上升                                                       | 只在需要 GeoJSON/聚类时引入，且只开 `geojson`（必要时 `proj`）feature                                    |
| gpui 0.2.2 版本较旧                  | API 受限、升级成本高                                                      | UI 逻辑集中在 `ui/`，core 不耦合 gpui                                                           |
| 5000+ 张两两比较                      | 相似分组不可用                                                           | pHash 预筛 + 缩略图缓存 + 后台线程                                                                |
| 误删照片                             | 不可逆数据丢失                                                           | 只进回收站、操作日志、预览影响范围、二次确认                                                                 |
| PRD 移动端信息架构与桌面实现不一致              | 返工                                                                | M0 前确认桌面优先假设                                                                           |
| 文档与源码漂移                          | 后续 Agent 读到过期信息                                                   | 结构性变更按 WORKFLOW.md 第 5/6 节同步模块 md 与 INDEX                                              |
| Claude.md 与 CODEBUDDY.md 双份维护    | 规则不一致                                                             | 修改任一份时同步另一份                                                                            |

## 9. 文档同步约定

- 本文件是规划文档，不按模块 md 模板维护，也不参与覆盖 hash。
- 每次里程碑若新增 `src/core/` 或 `src/ui/` 子模块，必须按 WORKFLOW.md 第 3 节模板生成对应模块 md，并刷新头部时间与 hash。
- M0 的目录分层与 M1 的数据流变化属于结构性变更，必须同步更新 `.docs/INDEX.md` 的目录职责与依赖图。
- 宣布里程碑完成前，按 WORKFLOW.md 第 6 节逐项自查，并在回复中说明更新了哪些 md 或“本次无需更新文档”。

## 10. 立即开始

建议从 M0 的第一件事开始：

1. `git init` 并提交当前原型作为基线（先确认 `.gitignore` 排除 `target/`）。
2. 建立 `src/core/`、`src/ui/` 骨架，把 `AppState`、`AppRoot`、页面函数从 `main.rs` 迁到 `ui/`。
3. 删除 `src/lib.rs` 的 `add()` 模板，改为 `pub mod core; pub mod ui;`，并同步更新模块文档。
4. 把 imageflow 改为 `zen-codecs` 纯 Rust 路径，移除 `imageflow_helpers` 直接依赖，并验证 `cargo check` 不再需要 NASM / VS Build Tools。
5. 用固定 commit `7cbe26be` 的 git submodule 引入 imageflow（AGPLv3 个人使用已确认可接受），不再评估 HEIC 依赖。
6. 照片足迹本期暂缓：不实现 EXIF GPS → GCJ-02 → 地图链路，不引入 OxiGeo / 地图 SDK / GCJ-02 依赖；相关规划保留在 M4+，待整理链路闭环后再评估启动。
7. 进入 M1；“桌**面优先”与“个人本地使用**”两项前提已确认。
