# 模块：core-edit

> 生成时间：2026-11-23 12:05 +0800
> 覆盖范围：src/core/edit.rs（按路径排序）
> 覆盖 hash：9ab5108e3a22（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责

基于 imageflow（固定 commit submodule）的本地图片编辑流水线。把一组编辑参数转换成 imageflow 图节点并执行；
另存为输出无损 PNG，交互预览输出 JPEG 并在解码阶段降采样。纯 `core` 模块，不依赖 gpui，可独立单元测试。

## 对外接口

- `pub enum OutputFormat { Png, Jpeg }`：输出格式；`extension()` 为私有辅助，分别返回 `png` / `jpg`。
- `pub enum ColorFilter { None, Grayscale, Sepia, Invert }`：颜色滤镜，映射 `imageflow_types::ColorFilterSrgb`。
- `pub struct EditOptions`：可克隆/Copy/Send 的编辑参数。
  - `rotate_degrees: i32`（规范化为 0/90/180/270）
  - `flip_h: bool` / `flip_v: bool`
  - `filter: ColorFilter`
  - `brightness: f32` / `contrast: f32` / `saturation: f32`（1.0 为原图，钳制不小于 0）
  - `max_dimension: u32`（0 表示不缩放；预览用 1600，另存为用 0 保留原分辨率）
  - `pub fn has_user_edits(&self) -> bool`：忽略 `max_dimension` 后是否与默认参数不同。
  - `pub fn is_identity(&self) -> bool`：是否与原图完全一致（含 `max_dimension == 0`）；UI 恢复默认参数时用其命中底图快路径。
- `pub fn apply(src: &Path, dst: &Path, options: &EditOptions) -> anyhow::Result<()>`：
  读取 `src`，按 `options.steps(OutputFormat::Png, None)` 构造 imageflow 图并执行，结果以 PNG 写入 `dst`。
  同步阻塞，调用方需放到后台执行器；写盘走「同目录临时文件 + rename」，失败时清理临时文件。
- `pub fn apply_preview(src: &Path, dst: &Path, options: &EditOptions) -> anyhow::Result<()>`：
  同一条编辑流水线，但输出 JPEG（quality 88）；当 `max_dimension > 0` 时先读图像信息生成
  `JpegIDCTDownscaleHints`，让解码器在 IDCT 阶段直接缩小，避免先全分辨率解码再缩放。
- `pub fn preview_path(base: &Path, src: &Path, options: &EditOptions, format: OutputFormat) -> PathBuf`：
  按「源路径 + 参数调试串 + 输出格式」做 `XxHash64`，生成 `edit-<hash>.png` 或 `edit-<hash>.jpg`，
  保证不同参数/格式互不覆盖，避免后台任务乱序返回时写坏预览文件。

## 内部关键流程

1. `apply` / `apply_preview` 汇入 `apply_inner(src, dst, options, format)`。
2. `apply_inner` 校验源文件存在 → 创建 `dst` 父目录 → `Context::create()` →
   `ctx.add_file(0, IoDirection::In, src)` 以文件流装载（不再整图读入内存）。
3. 仅 JPEG 且 `max_dimension > 0` 时调用 `get_unscaled_rotated_image_info(0)`，按
   `max / max(w, h)` 计算 `JpegIDCTDownscaleHints`（`scale_luma_spatially` 与 gamma 校正均开启）。
4. `ctx.add_output_buffer(1)` → `execute_1(Execute001 { framewise: Steps(...) })` →
   `take_output_buffer(1)` → 非空校验。
5. `options.steps(format, decode_hints)`：`Decode(io_id=0, commands=hints)` → `Rotate90/180/270`
   → `FlipH` → `FlipV` → `ColorFilterSrgb` 滤镜 → 亮度/对比度/饱和度（偏离 1.0 才加入）
   → `Constrain(Within, max_dimension)`（>0 才加入）→ `Encode(io_id=1, preset)`。
   `OutputFormat::Png` 用 `Lodepng { maximum_deflate: false }`，`Jpeg` 用 `LibjpegTurbo { quality: 88, ... }`。
6. `write_atomic(dst, output)`：写 `.tmp-<pid>-<name>` → `rename` 到 `dst`；Windows 目标被占用时退化为 `copy` + 删除临时文件。
7. 单元测试：恒等编辑保尺寸、旋转交换宽高、翻转+滤镜+调整+缩放、`is_identity`/`has_user_edits` 边界、
   缺失源文件拒绝、预览路径随参数/格式变化、预览走 JPEG 且缩小到最长边；
   另有 `#[ignore]` 基准 `preview_performance_comparison` 对比旧 PNG 预览与新 JPEG 预览耗时。

## 依赖关系

- 依赖的外部模块：`imageflow_core`（`Context::create` / `add_file` / `get_unscaled_rotated_image_info` /
  `add_output_buffer` / `execute_1` / `take_output_buffer` / `IoDirection`）、
  `imageflow_types`（`Node` / `Framewise` / `Execute001` / `Constraint` / `EncoderPreset` /
  `DecoderCommand` / `JpegIDCTDownscaleHints` / `ColorFilterSrgb`）、`anyhow`、`twox-hash`；
  测试依赖 `image`。
- 被谁依赖：`ui-desktop`（`src/ui/editor.rs` 的底图生成、交互预览与另存为）。

## 常见改动点

- 新增编辑能力：在 `EditOptions` 加字段，并在 `steps()` 对应位置插入 imageflow 节点；`preview_path` 用参数调试串，自动跟随。
- 更换输出格式：修改 `OutputFormat::extension()` 与 `steps()` 末尾的 `Encode` preset。
- 调整预览分辨率：改 `src/ui/editor.rs::PREVIEW_MAX_DIMENSION`；本模块只接收 `max_dimension`。
- 调整预览速度：改 `LibjpegTurbo` 的 quality 或 `JpegIDCTDownscaleHints` 的缩放策略；另存为仍走 PNG 无损路径。
- 性能基准：`cargo test --lib preview_performance_comparison -- --ignored --nocapture`。

## 已知风险 / 易错点

- `apply` / `apply_preview` 都是同步阻塞调用，必须放到后台执行器；`Context` 含 `RefCell` 不是 `Send`，
  必须在后台任务内部创建，不能跨线程共享实例。
- `preview_path` 的文件扩展名由 `OutputFormat` 决定：调用 `apply_preview` 时必须传 `OutputFormat::Jpeg`，
  否则缓存文件后缀与实际编码不一致。
- `JpegIDCTDownscaleHints` 只对 JPEG 输入有效；PNG/其他格式预览时 `max_dimension` 仍由 `Constrain` 节点兜底。
- `is_identity()` 包含 `max_dimension`，所以「仅设了预览尺寸」的参数不会命中 UI 底图快路径；
  「重置全部编辑」必须恢复完整 `EditOptions::default()` 才能零计算复用底图。
- 另存为用 `max_dimension: 0` 保留原分辨率；若误传预览尺寸会丢失原图像素。
- 图片吞吐强依赖构建模式：dev 下未优化 imageflow/mozjpeg 会让首张预览明显变慢，已在 `Cargo.toml`
  用 `[profile.dev.package.*] opt-level = 3` 覆盖；发布仍推荐 `cargo build --release`。

## 变更记录

- 2026-11-23 12:05：补齐 `OutputFormat` / `apply_preview` / JPEG IDCT 降采样与性能基准文档；新增 `is_identity` / `has_user_edits` 边界测试；刷新覆盖 hash。
- 2026-11-20 17:03：初版基于 imageflow 的 PNG 编辑流水线与预览缓存路径。
