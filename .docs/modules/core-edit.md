# 模块：core-edit

> 生成时间：2026-11-20 17:03 +0800
> 覆盖范围：src/core/edit.rs（按路径排序）
> 覆盖 hash：04ddd36ec5af（取前 12 位，由生成者用命令行工具计算）
> 可信度：AI 生成未审
> 状态：valid

## 职责

基于 imageflow（固定 commit submodule）的本地图片编辑流水线。把一组编辑参数转换成 imageflow 图节点并执行，
输出 PNG；纯 `core` 模块，不依赖 gpui，可独立单元测试。

## 对外接口

- `pub enum ColorFilter { None, Grayscale, Sepia, Invert }`：颜色滤镜，映射 `imageflow_types::ColorFilterSrgb`。
- `pub struct EditOptions`：可克隆/Send 的编辑参数。
  - `rotate_degrees: i32`（规范化为 0/90/180/270）
  - `flip_h: bool` / `flip_v: bool`
  - `filter: ColorFilter`
  - `brightness: f32` / `contrast: f32` / `saturation: f32`（1.0 为原图，钳制不小于 0）
  - `max_dimension: u32`（0 表示不缩放）
  - `pub fn has_user_edits(&self) -> bool`：忽略 `max_dimension` 后是否与默认参数不同。
  - `pub fn is_identity(&self) -> bool`：是否与默认值完全相同。
- `pub fn apply(src: &Path, dst: &Path, options: &EditOptions) -> anyhow::Result<()>`：
  读取 `src`，按 `options.steps()` 构造 imageflow 图并执行，结果以 PNG 写入 `dst`。
  同步阻塞，调用方需放到后台执行器；写盘走「同目录临时文件 + rename」，失败时清理临时文件。
- `pub fn preview_path(base: &Path, src: &Path, options: &EditOptions) -> PathBuf`：
  按源路径与编辑节点的调试串做 `XxHash64`，生成 `edit-<hash>.png`，保证不同参数互不覆盖。

## 内部关键流程

1. `apply` 校验源文件存在 → 创建 `dst` 父目录 → `std::fs::read(src)`。
2. `Context::create()` → `add_input_vector(0, bytes)` → `add_output_buffer(1)`。
3. `options.steps()`：
   `Decode(io_id=0)` → `Rotate90/Rotate180/Rotate270` → `FlipH` → `FlipV` → `ColorFilterSrgb` 滤镜
   → 亮度/对比度/饱和度（仅在偏离 1.0 时加入）→ `Constrain(Within, max_dimension)`（仅 >0 时）
   → `Encode(io_id=1, Lodepng)`。
4. `execute_1(Execute001 { framewise: Steps(...) })` → `take_output_buffer(1)` → 非空校验。
5. 写 `.tmp-<pid>-<name>` → `rename` 到 `dst`；rename 失败退化为 `copy` + 删除临时文件。
6. 单元测试（`src/core/edit.rs` 内 `#[cfg(test)]`）：恒等编辑保尺寸、旋转交换宽高、翻转+滤镜+调整+缩放、
   缺失源文件拒绝、预览路径随参数变化；均使用 `image` crate 生成合成 PNG。

## 依赖关系

- 依赖的外部模块：`imageflow_core`（Context::create / execute_1 / add_input_vector / add_output_buffer / take_output_buffer）、
  `imageflow_types`（Node / Framewise / Execute001 / Constraint / EncoderPreset / ColorFilterSrgb）、
  `anyhow`、`twox-hash`；测试依赖 `image`。
- 被谁依赖：`ui-desktop`（`src/ui/editor.rs` 的预览与另存为）。

## 常见改动点

- 新增编辑能力：在 `EditOptions` 加字段，并在 `steps()` 对应位置插入 imageflow 节点；同步更新 `preview_path` 的哈希输入（节点调试串会自动包含）。
- 更换输出格式：修改 `steps()` 末尾的 `Encode` preset 与 `preview_path` 扩展名。
- 调整预览分辨率：修改 `src/ui/editor.rs` 中 `PhotoEditor::new` 的 `max_dimension`。

## 已知风险 / 易错点

- `apply` 是同步阻塞调用，直接在 UI 线程调用会卡住界面；必须放在后台执行器。
- `Context` 含 `RefCell`，不是 `Send`；必须在后台任务内部创建，不能跨线程共享实例。
- imageflow 输出统一为 PNG：若将来支持 JPEG/AVIF，需要同步修改 `Encode` preset 与文件扩展名。
- `max_dimension` 只用于预览；另存为必须显式置 0，否则会丢失原分辨率。
