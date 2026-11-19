# 模块：core-analyze

> 生成时间：2026-11-19 10:38 +0800
> 覆盖范围：src/core/analyze/blur.rs, src/core/analyze/classify.rs, src/core/analyze/mod.rs, src/core/analyze/pipeline.rs, src/core/analyze/similarity.rs, tests/analyze_test.rs（按路径排序）
> 覆盖 hash：a9809cab5871（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责
核心层的分析与智能整理子系统（M2）：模糊评分、截图识别、大文件排序、相似归组（pHash 预筛 + dssim 精排 + 代表元聚类）。不依赖 gpui，可独立单元测试，可在后台线程运行。2026-11-19 修复：坏图不写 `blur_score = 0.0`、纯色/低纹理图不产生伪 pHash、代表元聚类替代并查集传递闭包、截图启发式去掉 4:3 误判并加入精确屏幕尺寸权重、进度每 32 张节流上报。

## 对外接口
**mod.rs**（`pub use pipeline::*` 重导出分析入口）
- `pub fn analyze_library(conn: &Connection, progress: Option<&Sender<AnalyzeProgress>>) -> Result<AnalyzeReport>`：分析整库并落库。
- `pub fn analyze_photo(photo: &Photo) -> Option<PhotoAnalysis>`：分析单张（模糊/截图/pHash）；解码或评分失败返回 `None`，调用方不得写库。

**blur.rs**
- `pub const ANALYSIS_SIZE: u32 = 256`、`pub use crate::core::model::BLUR_THRESHOLD`（`f64 = 100.0`，常量定义在 core-model）。
- `pub fn blur_score(path: &Path) -> Result<f64>`、`pub fn is_blurry(score: f64) -> bool`、`pub fn laplacian_variance(gray: &GrayImage) -> f64`（0×0 / 1×1 图在缩放前直接返回 0.0）。

**classify.rs**
- `pub const SCREENSHOT_THRESHOLD: u32 = 50`。
- `pub fn screenshot_likelihood(photo: &Photo) -> u32`、`pub fn is_screenshot(photo: &Photo) -> bool`、`pub fn find_screenshots(photos: &[Photo]) -> Vec<&Photo>`、`pub fn find_large_files(photos: &[Photo], top_n: usize) -> Vec<&Photo>`。
- `SCREEN_RATIOS` 刻意不含 4:3 / 3:4（相机常见比例）；`SCREEN_SIZES` 精确屏幕分辨率命中加 30 分，让无关键词、无 EXIF 的真实截图可达阈值。

**similarity.rs**
- `pub const HAMMING_THRESHOLD: u32 = 8`、`pub const DSSIM_THRESHOLD: f64 = 0.03`。
- `pub fn phash_from_gray(gray: &GrayImage) -> Option<u64>`（AC 系数动态范围 < 1e-6 的纯色/低纹理图返回 `None`）、`pub fn phash_from_path(path: &Path) -> Result<u64>`、`pub fn hamming(a: u64, b: u64) -> u32`、`pub fn group_by_phash(items: &[(i64, u64)], max_dist: u32) -> Vec<Vec<i64>>`（代表元聚类，只返回成员 ≥2 的组）、`pub fn dssim_distance(a: &Path, b: &Path) -> Result<f64>`。

**pipeline.rs**
- `pub enum AnalyzeProgress`：`Started{total}` / `Analyzing{processed,total}` / `Finished{report}` / `Failed{message}`。
- `pub struct AnalyzeReport`：`photos_analyzed/photos_failed/blurry_count/screenshot_count/similar_groups/duration_ms`；`photos_failed` 含解码失败与写库失败两类。
- `pub struct PhotoAnalysis`：`photo_id/blur_score/is_screenshot/phash`。
- `const PROGRESS_EVERY: usize = 32`（模块内私有）：进度上报节流，避免 5000 张产生 5000 条消息。

## 内部关键流程
1. `analyze_library` → `db::list_photos` 取全部照片 → 逐张 `analyze_photo`：输入源优先缩略图（存在）否则原图 → `blur::blur_score`（失败即 `None`，计入 `photos_failed` 且保留 `blur_score` 为 NULL）+ `classify::is_screenshot` + `similarity::phash_from_path` → `db::update_analysis` 写回（失败也计入 `photos_failed`）→ 第 1 张及每 32 张上报一次 `Analyzing` 进度。
2. pHash：灰度缩放 32x32 → 可分离 1D DCT（行+列）→ 取左上 8x8 低频 → 检查 AC 系数动态范围，过小返回 `None` → 以 AC 均值（不含 DC）为阈值生成 64 位；汉明距离 ≤ 8 为候选。
3. 相似分组：`group_by_phash` 代表元聚类（A~B、B~C 但 A≁C 不串成一组）→ 每组 `refine_group` 用 dssim 精排（以第一张为基准，距离 > 0.03 剔除到 far 组递归；精排失败不并入核心组）；无缩略图时回退到纯 pHash 结果 → `db::save_similar_groups` 清空旧组重建（`created_at` 传 `db::now_millis()`，不是本次耗时）。
4. `db::list_photo_groups`（过滤成员 <2 的空组）/`list_blurry`（只返回 `blur_score < BLUR_THRESHOLD`，`id ASC` 稳定排序）/`list_screenshots`/`list_large_files` 供 UI `AnalyzeData::reload` 消费。

## 依赖关系
- 依赖的外部模块：`image`（灰度/缩放）、`dssim`（结构相似度，用其自带 load_image 从路径加载，避免 image 版本冲突）、`rusqlite`（core-scanner 的 db）、`anyhow`、`core-model`（Photo/PhotoGroup/BLUR_THRESHOLD）、`core-scanner`（db 读写）。
- 被谁依赖：`ui-desktop`（AppState::start_analysis 调用 analyze_library、AnalyzeData 消费结果）、集成测试 `tests/analyze_test.rs`。

## 常见改动点
- 调模糊/截图阈值：改 core-model 的 `BLUR_THRESHOLD` / classify 的 `SCREENSHOT_THRESHOLD` 并补测试样本。
- 改相似敏感度：调 `HAMMING_THRESHOLD` / `DSSIM_THRESHOLD`；注意 5000 张场景的 O(n²) pHash 比较成本。
- 加新分析维度：在 `PhotoAnalysis` 加字段 → `analyze_photo` 计算 → `db::update_analysis` + schema 列 → `AnalyzeReport` 统计。
- 换 dssim 实现或加载方式：只改 `dssim_distance`。
- 加截图判定证据：改 `SCREEN_RATIOS` / `SCREEN_SIZES` / 关键词表；注意 4:3 是相机比例，不可当截图证据。

## 已知风险 / 易错点
- **测试样本频率**：周期 2 的棋盘格是病态高频样本，降采样（缩略图→256px）会被 Triangle 滤波抹平导致误判为模糊；清晰样本用周期 8 条纹或真实照片。`laplacian_variance` 阈值针对 256px 缩放设定。
- `group_by_phash` 为 O(n²) 两两比较，5000 张约 1250 万次汉明距离（轻量），但 dssim 精排仅对候选组内进行以控成本；代表元聚类避免传递闭包串成巨组。
- `save_similar_groups` 自带事务，不可在 scanner 已持事务时调用（本次 analysis 与 scan 在不同 DB 连接/线程）。
- dssim 3.4 的 `f64::from(val)` 取距离；其 `load_image` 会做色彩配置转换，坏图会 Err，`refine_group` 中精排失败按「不并入核心组」处理。
- **坏图不得写成 `blur_score = 0.0`**：否则会被当成已分析、永远排在模糊列表最前面并污染 `count_analyzed`；`analyze_photo` 返回 `None` 时保留 NULL 便于下次重试。
- **截图启发式**：4:3 / 3:4 属相机常见比例，不能作为截图证据；精确屏幕分辨率是最强证据，可让无关键词截图达标。
- `phash` 为 u64，写入 SQLite INTEGER 时按位转 i64，读出 `as u64` 还原（见 db::row_to_photo）。
- 分析输入源优先 400px 缩略图，模糊阈值针对 256px 缩放标定，换图源需重新标定。
- 本模块不得引入 gpui 依赖。

## 变更记录
- 2026-11-19 10:38：全量 bug 审查修复（坏图不落库、pHash 低纹理返回 `None`、代表元聚类替代并查集、截图 4:3 误判与精确屏幕尺寸加权、`BLUR_THRESHOLD` 共用常量、进度节流、dssim 失败不判相似、`list_blurry` 加阈值与稳定排序）；新增 3 个集成回归测试 + 2 个截图单测；刷新 hash。
- 2026-11-18 15:40：M2 新建模块，实现模糊评分、截图识别、大文件排序、pHash+dssim 相似归组与分析编排/进度。
