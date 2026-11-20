//! 图片编辑内核：基于 imageflow（固定 commit submodule）的本地编辑流水线。
//!
//! 纯 core 模块，不依赖 gpui，可独立单元测试。编辑结果只写本机磁盘，不上传。
//! 预览与另存为走同一条流水线，区别仅是预览可限制最大边长以加快响应。

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use imageflow_core::{Context, IoDirection};
use imageflow_types as s;

/// 输出格式：预览用 JPEG（编码快、文件小），另存为用 PNG（无损）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    Png,
    Jpeg,
}

impl OutputFormat {
    fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
        }
    }
}

/// 颜色滤镜（直接映射 imageflow 的 `ColorFilterSrgb`）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorFilter {
    #[default]
    None,
    Grayscale,
    Sepia,
    Invert,
}

impl ColorFilter {
    fn node(self) -> Option<s::Node> {
        let filter = match self {
            ColorFilter::None => return None,
            ColorFilter::Grayscale => s::ColorFilterSrgb::GrayscaleNtsc,
            ColorFilter::Sepia => s::ColorFilterSrgb::Sepia,
            ColorFilter::Invert => s::ColorFilterSrgb::Invert,
        };
        Some(s::Node::ColorFilterSrgb(filter))
    }
}

/// 一组可重复应用的编辑参数（Clone + Send，便于从 UI 线程交给后台任务）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditOptions {
    /// 顺时针旋转角度，规范化为 0 / 90 / 180 / 270。
    pub rotate_degrees: i32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub filter: ColorFilter,
    /// 亮度倍率，1.0 为原图。
    pub brightness: f32,
    /// 对比度倍率，1.0 为原图。
    pub contrast: f32,
    /// 饱和度倍率，1.0 为原图。
    pub saturation: f32,
    /// 限制输出最长边（0 表示不缩放）。预览用 1600，另存为用 0 保留原分辨率。
    pub max_dimension: u32,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            rotate_degrees: 0,
            flip_h: false,
            flip_v: false,
            filter: ColorFilter::None,
            brightness: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            max_dimension: 0,
        }
    }
}

impl EditOptions {
    /// 用户是否做了实际编辑（忽略仅用于预览的 max_dimension）。
    pub fn has_user_edits(&self) -> bool {
        let base = Self {
            max_dimension: self.max_dimension,
            ..Default::default()
        };
        *self != base
    }

    /// 是否与原图完全一致（无需调用 imageflow）。
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }

    fn normalized_rotation(&self) -> i32 {
        let r = self.rotate_degrees.rem_euclid(360);
        if r >= 270 {
            270
        } else if r >= 180 {
            180
        } else if r >= 90 {
            90
        } else {
            0
        }
    }

    /// 构造 imageflow 图节点。
    ///
    /// `decode_hints` 只在 JPEG 预览且 `max_dimension > 0` 时提供，让解码器直接输出
    /// 接近目标尺寸的小图，避免先全分辨率解码再缩小的开销。
    fn steps(
        &self,
        format: OutputFormat,
        decode_hints: Option<s::JpegIDCTDownscaleHints>,
    ) -> Vec<s::Node> {
        let mut nodes = Vec::new();
        // EXIF 方向由 imageflow 解码阶段自动应用（默认行为），这里只叠加用户显式编辑。
        nodes.push(s::Node::Decode {
            io_id: 0,
            commands: decode_hints.map(|h| vec![s::DecoderCommand::JpegDownscaleHints(h)]),
        });
        match self.normalized_rotation() {
            90 => nodes.push(s::Node::Rotate90),
            180 => nodes.push(s::Node::Rotate180),
            270 => nodes.push(s::Node::Rotate270),
            _ => {}
        }
        if self.flip_h {
            nodes.push(s::Node::FlipH);
        }
        if self.flip_v {
            nodes.push(s::Node::FlipV);
        }
        if let Some(node) = self.filter.node() {
            nodes.push(node);
        }
        if (self.brightness - 1.0).abs() > f32::EPSILON {
            nodes.push(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Brightness(
                self.brightness.max(0.0),
            )));
        }
        if (self.contrast - 1.0).abs() > f32::EPSILON {
            nodes.push(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Contrast(
                self.contrast.max(0.0),
            )));
        }
        if (self.saturation - 1.0).abs() > f32::EPSILON {
            nodes.push(s::Node::ColorFilterSrgb(s::ColorFilterSrgb::Saturation(
                self.saturation.max(0.0),
            )));
        }
        if self.max_dimension > 0 {
            nodes.push(s::Node::Constrain(s::Constraint {
                mode: s::ConstraintMode::Within,
                w: Some(self.max_dimension),
                h: Some(self.max_dimension),
                hints: None,
                gravity: None,
                canvas_color: None,
            }));
        }
        let preset = match format {
            OutputFormat::Png => s::EncoderPreset::Lodepng {
                maximum_deflate: Some(false),
            },
            // 预览用 libjpeg-turbo 直接出 JPEG：比逐像素 PNG 编码快得多，文件也更小。
            OutputFormat::Jpeg => s::EncoderPreset::LibjpegTurbo {
                quality: Some(88),
                progressive: Some(false),
                optimize_huffman_coding: Some(false),
                matte: None,
            },
        };
        nodes.push(s::Node::Encode { io_id: 1, preset });
        nodes
    }
}

fn apply_inner(
    src: &Path,
    dst: &Path,
    options: &EditOptions,
    format: OutputFormat,
) -> Result<()> {
    if !src.is_file() {
        bail!("原图不存在: {}", src.display());
    }
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("创建编辑输出目录失败: {}", parent.display()))?;

    // 直接交给 imageflow 按文件流读取，避免先把整张原图读进内存再复制进 Context。
    let src_str = src
        .to_str()
        .with_context(|| format!("原图路径不是 UTF-8: {}", src.display()))?;
    let mut ctx = Context::create().context("初始化 imageflow 上下文失败")?;
    ctx.add_file(0, IoDirection::In, src_str)
        .with_context(|| format!("imageflow 装载原图失败: {}", src.display()))?;

    // JPEG 预览：让解码器在 IDCT 阶段直接缩小，避免「全分辨率解码 → 再缩放」的双重开销。
    let decode_hints = if format == OutputFormat::Jpeg && options.max_dimension > 0 {
        ctx.get_unscaled_rotated_image_info(0).ok().map(|info| {
            let w = info.image_width.max(1) as f64;
            let h = info.image_height.max(1) as f64;
            let max = options.max_dimension as f64;
            let scale = (max / w.max(h)).min(1.0);
            s::JpegIDCTDownscaleHints {
                width: (w * scale).round().max(1.0) as i64,
                height: (h * scale).round().max(1.0) as i64,
                scale_luma_spatially: Some(true),
                gamma_correct_for_srgb_during_spatial_luma_scaling: Some(true),
            }
        })
    } else {
        None
    };

    ctx.add_output_buffer(1)
        .context("imageflow 创建输出缓冲区失败")?;
    ctx.execute_1(s::Execute001 {
        job_options: None,
        graph_recording: None,
        security: None,
        framewise: s::Framewise::Steps(options.steps(format, decode_hints)),
    })
    .context("imageflow 执行编辑失败")?;
    let output = ctx
        .take_output_buffer(1)
        .context("imageflow 取回编辑结果失败")?;
    if output.is_empty() {
        bail!("imageflow 输出为空: {}", src.display());
    }

    write_atomic(dst, &output)
}

/// 把 `src` 按 `options` 编辑后写成无损 PNG 到 `dst`（用于另存为）。
///
/// 输出先写同目录临时文件再原子改名，避免中途失败留下半截图片被 UI 当作有效结果。
/// 同步阻塞调用，必须由调用方放到后台线程/执行器上执行。
pub fn apply(src: &Path, dst: &Path, options: &EditOptions) -> Result<()> {
    apply_inner(src, dst, options, OutputFormat::Png)
}

/// 快速预览：与 `apply` 同一条编辑流水线，但输出 JPEG，并在 JPEG 解码阶段
/// 按 `max_dimension` 直接降采样。调用方仍需放在后台执行器上。
pub fn apply_preview(src: &Path, dst: &Path, options: &EditOptions) -> Result<()> {
    apply_inner(src, dst, options, OutputFormat::Jpeg)
}

/// 编辑结果的缓存路径：按「源文件路径 + 参数指纹 + 输出格式」命名。
/// 不同参数互不覆盖，避免后台任务乱序返回时把旧预览写到新预览文件上。
pub fn preview_path(
    base: &Path,
    src: &Path,
    options: &EditOptions,
    format: OutputFormat,
) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = twox_hash::XxHash64::default();
    src.to_string_lossy().as_bytes().hash(&mut hasher);
    format!("{options:?}").hash(&mut hasher);
    format.hash(&mut hasher);
    base.join(format!("edit-{:016x}.{}", hasher.finish(), format.extension()))
}

fn write_atomic(dst: &Path, output: &[u8]) -> Result<()> {
    let tmp = temp_output_path(dst);
    // 清理上次崩溃遗留的同名临时文件，write 会覆盖已有内容，这里主要避免误用旧文件。
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, output)
        .with_context(|| format!("写入编辑结果失败: {}", tmp.display()))?;
    match std::fs::rename(&tmp, dst) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Windows 下目标被占用时，改为复制后删除临时文件，仍能保证 dst 是完整文件。
            match std::fs::copy(&tmp, dst) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&tmp);
                    Ok(())
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    Err(e).with_context(|| format!("提交编辑结果失败: {}", dst.display()))
                }
            }
        }
    }
}

fn temp_output_path(dst: &Path) -> PathBuf {
    let name = dst
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "edit.png".to_string());
    let tmp_name = format!(".tmp-{}-{name}", std::process::id());
    dst.with_file_name(tmp_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, ImageBuffer, Rgb};

    fn sample_png(path: &Path, w: u32, h: u32) {
        let img = ImageBuffer::from_fn(w, h, |x, y| Rgb([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            128,
        ]));
        image::save_buffer_with_format(path, img.as_raw(), w, h, image::ColorType::Rgb8, image::ImageFormat::Png)
            .unwrap();
    }

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "akashic-edit-test-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn identity_edit_keeps_dimensions_and_writes_png() {
        let dir = TempDir::new("identity");
        let src = dir.path().join("src.png");
        let dst = dir.path().join("dst.png");
        sample_png(&src, 32, 16);
        apply(&src, &dst, &EditOptions::default()).unwrap();
        let decoded = image::open(&dst).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (32, 16));
        assert!(std::fs::metadata(&dst).unwrap().len() > 0);
    }

    #[test]
    fn rotation_swaps_dimensions() {
        let dir = TempDir::new("rotate");
        let src = dir.path().join("src.png");
        let dst = dir.path().join("dst.png");
        sample_png(&src, 40, 20);
        let opts = EditOptions {
            rotate_degrees: 90,
            ..Default::default()
        };
        apply(&src, &dst, &opts).unwrap();
        let decoded = image::open(&dst).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (20, 40));
    }

    #[test]
    fn flip_filter_and_adjustments_are_applied() {
        let dir = TempDir::new("filters");
        let src = dir.path().join("src.png");
        let dst = dir.path().join("dst.png");
        sample_png(&src, 24, 12);
        let opts = EditOptions {
            rotate_degrees: 0,
            flip_h: true,
            flip_v: true,
            filter: ColorFilter::Sepia,
            brightness: 1.1,
            contrast: 1.2,
            saturation: 0.8,
            max_dimension: 16,
        };
        apply(&src, &dst, &opts).unwrap();
        let decoded = image::open(&dst).unwrap().to_rgb8();
        let (w, h) = decoded.dimensions();
        assert!(w <= 16 && h <= 16, "max_dimension 应限制最长边, got {w}x{h}");
        assert_eq!(w * 1, h * 2, "等比缩放应保持 2:1 比例, got {w}x{h}");
    }

    #[test]
    fn missing_source_is_rejected() {
        let dir = TempDir::new("missing");
        let err = apply(
            &dir.path().join("nope.png"),
            &dir.path().join("dst.png"),
            &EditOptions::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("原图不存在"));
    }

    #[test]
    fn preview_path_changes_with_options_and_format() {
        let base = PathBuf::from("/tmp/akashic-editor");
        let src = PathBuf::from("/photos/a.png");
        let a = preview_path(&base, &src, &EditOptions::default(), OutputFormat::Jpeg);
        let b = preview_path(
            &base,
            &src,
            &EditOptions {
                rotate_degrees: 90,
                ..Default::default()
            },
            OutputFormat::Jpeg,
        );
        let png = preview_path(&base, &src, &EditOptions::default(), OutputFormat::Png);
        assert_ne!(a, b);
        assert_ne!(a, png);
        assert_eq!(a.parent(), Some(base.as_path()));
        assert_eq!(a.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(png.extension().and_then(|e| e.to_str()), Some("png"));
    }

    #[test]
    fn preview_uses_jpeg_and_decoder_downscale_hints() {
        let dir = TempDir::new("preview-jpeg");
        let src = dir.path().join("src.png");
        let dst = dir.path().join("preview.jpg");
        sample_png(&src, 400, 200);
        let opts = EditOptions {
            max_dimension: 64,
            ..Default::default()
        };
        apply_preview(&src, &dst, &opts).unwrap();
        let decoded = image::open(&dst).unwrap();
        let (w, h) = decoded.dimensions();
        assert!(w <= 64 && h <= 64, "预览应缩小到最长边 64, got {w}x{h}");
        assert_eq!(w, h * 2, "等比缩放应保持 2:1, got {w}x{h}");
        assert!(std::fs::metadata(&dst).unwrap().len() > 0);
    }

    /// 手动性能对比：旧预览（全分辨率解码 + PNG 编码）vs 新预览（JPEG 解码降采样 + JPEG 编码）。
    /// 用 `cargo test --lib preview_performance_comparison -- --ignored --nocapture` 运行。
    #[test]
    #[ignore = "性能基准：需要时手动运行"]
    fn preview_performance_comparison() {
        let dir = TempDir::new("perf");
        let src = dir.path().join("large.jpg");
        let img = ImageBuffer::from_fn(4000, 3000, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        image::save_buffer_with_format(
            &src,
            img.as_raw(),
            4000,
            3000,
            image::ColorType::Rgb8,
            image::ImageFormat::Jpeg,
        )
        .unwrap();

        let opts = EditOptions {
            rotate_degrees: 90,
            filter: ColorFilter::Sepia,
            max_dimension: 1600,
            ..Default::default()
        };
        let old_dst = dir.path().join("old-preview.png");
        let new_dst = dir.path().join("new-preview.jpg");

        let t0 = std::time::Instant::now();
        apply(&src, &old_dst, &opts).unwrap();
        let old_ms = t0.elapsed().as_millis();

        let t1 = std::time::Instant::now();
        apply_preview(&src, &new_dst, &opts).unwrap();
        let new_ms = t1.elapsed().as_millis();

        println!(
            "4000x3000 JPEG -> 1600px: old PNG preview {}ms, new JPEG preview {}ms",
            old_ms, new_ms
        );

        // 第二次之后的交互都在这张 1600px 底图上运行，才是用户实际感受到的速度。
        let edit_dst = dir.path().join("edit-preview.jpg");
        let t2 = std::time::Instant::now();
        apply_preview(&new_dst, &edit_dst, &opts).unwrap();
        println!("1600px base -> edited preview: {}ms", t2.elapsed().as_millis());
        assert!(edit_dst.is_file());
    }
}
