//! 原图编辑浮窗：点击照片后以独立窗口展示快速预览，并用 imageflow 做本地编辑。
//!
//! 编辑能力：旋转 / 翻转 / 灰度·棕褐·反色 / 亮度·对比度·饱和度 / 等比缩放。
//! 性能策略：打开时只从原图生成一次 1600px JPEG 底图，后续交互都在底图上处理；
//! 连续点击做 80ms 合并 + 单飞行任务，避免重复全分辨率解码/PNG 编码；
//! 「另存为」才按原分辨率输出 PNG。

use std::path::PathBuf;
use std::time::Duration;

use gpui::*;
use gpui_component::Root;
use gpui_component::button::Button;

use crate::core::config::Paths;
use crate::core::edit::{
    ColorFilter, EditOptions, OutputFormat, apply, apply_preview, preview_path,
};
use crate::core::model::Photo;
use crate::ui::window_titlebar;

const BG_WHITE: u32 = 0xFFFFFF;
const BG_WARM: u32 = 0xF5F2ED;
const TEXT_PRIMARY: u32 = 0x1A2634;
const TEXT_SECONDARY: u32 = 0x5A6A7A;
const BORDER_LIGHT: u32 = 0xE8E4DE;
const GOLD: u32 = 0xC9A96E;
const DANGER: u32 = 0xE0544C;

/// 交互预览的最长边。底图与编辑预览都用这个尺寸，足够 1100×760 浮窗显示。
const PREVIEW_MAX_DIMENSION: u32 = 1600;
/// 连续点击合并窗口：只重跑最后一次参数，避免每个 +/- 点击都触发一次全流水线。
const PREVIEW_DEBOUNCE_MS: u64 = 80;

fn col(hex: u32) -> gpui::Hsla {
    gpui::Hsla::from(gpui::rgb(hex))
}

pub struct PhotoEditor {
    photo: Photo,
    source: PathBuf,
    paths: Paths,
    label: String,
    /// 当前展示的图片路径：优先缓存底图/缩略图，其次原图；编辑成功后指向结果图。
    preview: PathBuf,
    /// 由原图生成的一次性快速底图；后续编辑都基于它，避免反复处理全分辨率原图。
    base: Option<PathBuf>,
    base_started: bool,
    base_ready: bool,
    options: EditOptions,
    status: String,
    error: Option<String>,
    busy: bool,
    /// 后台任务代次：旧任务返回时若代次已变则丢弃，避免慢任务覆盖新预览。
    generation: u64,
    /// 预览任务正在运行（含 debounce 等待）；新变更先记 pending，完成后只重跑最后一次。
    preview_running: bool,
    pending_preview: bool,
    /// 另存为单独计数，避免它取消正在进行的预览任务。
    save_running: bool,
}

impl PhotoEditor {
    fn new(photo: Photo, paths: Paths) -> Self {
        let source = photo.path.clone();
        let label = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名照片".to_string());

        let preview_dir = paths.data_dir.join("editor");
        let cached_base = preview_path(
            &preview_dir,
            &source,
            &Self::base_options(),
            OutputFormat::Jpeg,
        );
        // 缓存底图仍然新于原图时直接复用，重开浮窗无需再解码一次大图。
        let cached_ok = cached_base.is_file()
            && match (
                std::fs::metadata(&cached_base).and_then(|m| m.modified()),
                std::fs::metadata(&source).and_then(|m| m.modified()),
            ) {
                (Ok(base_time), Ok(src_time)) => base_time >= src_time,
                _ => false,
            };
        let initial = if cached_ok {
            cached_base.clone()
        } else {
            photo
                .thumbnail_path
                .clone()
                .filter(|p| p.is_file())
                .unwrap_or_else(|| source.clone())
        };
        let status = if !source.is_file() {
            "原图文件已移动或删除，编辑不可用".to_string()
        } else if cached_ok {
            "快速预览已就绪（最长边 1600px）".to_string()
        } else {
            "正在准备快速预览（最长边 1600px）…".to_string()
        };

        Self {
            preview: initial,
            base: cached_ok.then_some(cached_base),
            base_started: cached_ok,
            base_ready: cached_ok,
            photo,
            source,
            paths,
            label,
            options: EditOptions::default(),
            status,
            error: None,
            busy: false,
            generation: 0,
            preview_running: false,
            pending_preview: false,
            save_running: false,
        }
    }

    fn base_options() -> EditOptions {
        EditOptions {
            max_dimension: PREVIEW_MAX_DIMENSION,
            ..Default::default()
        }
    }

    fn preview_dir(&self) -> PathBuf {
        self.paths.data_dir.join("editor")
    }

    /// 浮窗创建后调用：若没有可复用底图则启动一次性构建。
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if !self.base_ready {
            self.ensure_base(cx);
        }
    }

    fn ensure_base(&mut self, cx: &mut Context<Self>) {
        if self.base_started {
            return;
        }
        self.base_started = true;
        if !self.source.is_file() {
            self.base_ready = true;
            self.error = Some("原图文件不存在，无法编辑".to_string());
            cx.notify();
            return;
        }
        self.busy = true;
        self.status = "正在准备 1600px 快速预览…".to_string();

        let src = self.source.clone();
        let dest = preview_path(
            &self.preview_dir(),
            &self.source,
            &Self::base_options(),
            OutputFormat::Jpeg,
        );
        let options = Self::base_options();
        let task_dest = dest.clone();
        let task = cx
            .background_executor()
            .spawn(async move { apply_preview(&src, &task_dest, &options) });
        cx.spawn(async move |this, app| {
            let result = task.await;
            let _ = this.update(app, |this, cx| {
                match result {
                    Ok(()) => {
                        this.base = Some(dest.clone());
                        this.base_ready = true;
                        this.preview = dest;
                        this.status = "快速预览已就绪（最长边 1600px）".to_string();
                        this.error = None;
                    }
                    Err(e) => {
                        // 底图失败不应让编辑不可用：回退原图，后续编辑仍在后台执行。
                        this.base = Some(this.source.clone());
                        this.base_ready = true;
                        this.preview = this.source.clone();
                        this.status = "已回退原图预览".to_string();
                        this.error = Some(format!("快速预览生成失败：{e:#}"));
                    }
                }
                this.busy = this.preview_running || this.save_running;
                if this.pending_preview {
                    this.pending_preview = false;
                    let generation = this.generation;
                    this.spawn_preview(generation, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn mutate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut EditOptions)) {
        f(&mut self.options);
        self.schedule_preview(cx);
    }

    fn schedule_preview(&mut self, cx: &mut Context<Self>) {
        if !self.source.is_file() {
            self.error = Some("原图文件不存在，无法编辑".to_string());
            cx.notify();
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.busy = true;
        self.status = "正在生成预览 …".to_string();
        self.error = None;

        if !self.base_ready {
            // 底图还没好：先启动底图，底图完成后再跑最新参数。
            self.pending_preview = true;
            self.ensure_base(cx);
            cx.notify();
            return;
        }
        self.spawn_preview(generation, cx);
        cx.notify();
    }

    /// 启动一次预览任务；若已有任务在飞，只登记 pending，完成后重跑最新参数。
    fn spawn_preview(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.preview_running {
            self.pending_preview = true;
            return;
        }
        let src = self.base.clone().unwrap_or_else(|| self.source.clone());
        let options = self.options;
        let dest = preview_path(&self.preview_dir(), &src, &options, OutputFormat::Jpeg);
        self.preview_running = true;

        cx.spawn(async move |this, app| {
            // 80ms 内连续点击合并为一次，等待期间若参数已变则直接退出。
            gpui::background_executor()
                .timer(Duration::from_millis(PREVIEW_DEBOUNCE_MS))
                .await;
            let superseded = this
                .update(app, |this, _| this.generation != generation)
                .unwrap_or(true);
            if superseded {
                let _ = this.update(app, |this, cx| {
                    this.preview_running = false;
                    this.busy = this.preview_running || this.save_running;
                    if this.pending_preview {
                        this.pending_preview = false;
                        let generation = this.generation;
                        this.spawn_preview(generation, cx);
                    }
                    cx.notify();
                });
                return;
            }

            let task_dest = dest.clone();
            let result = gpui::background_executor()
                .spawn(async move { apply_preview(&src, &task_dest, &options) })
                .await;
            let _ = this.update(app, |this, cx| {
                this.preview_running = false;
                if this.generation == generation {
                    match result {
                        Ok(()) => {
                            this.preview = dest;
                            this.status = if this.options.has_user_edits() {
                                "已应用编辑 · 快速预览（最长边 1600px）".to_string()
                            } else {
                                "快速预览已就绪（最长边 1600px）".to_string()
                            };
                            this.error = None;
                        }
                        Err(e) => {
                            this.error = Some(format!("编辑失败：{e:#}"));
                            this.status = "预览失败，仍显示上一张图片".to_string();
                        }
                    }
                }
                this.busy = this.preview_running || this.save_running;
                if this.pending_preview {
                    this.pending_preview = false;
                    let generation = this.generation;
                    this.spawn_preview(generation, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_as(&mut self, cx: &mut Context<Self>, dest: PathBuf) {
        if !self.source.is_file() {
            self.error = Some("原图文件不存在，无法另存为".to_string());
            cx.notify();
            return;
        }
        self.save_running = true;
        self.busy = true;
        self.status = format!("正在保存原分辨率 PNG 到 {} …", dest.display());
        self.error = None;

        let src = self.source.clone();
        let options = self.options;
        let staged = preview_path(
            &self.preview_dir(),
            &self.source,
            &options,
            OutputFormat::Png,
        );
        let target = dest.clone();
        let task = cx.background_executor().spawn(async move {
            apply(&src, &staged, &options)?;
            std::fs::copy(&staged, &target)
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("写入另存为文件失败：{e}"))
        });
        cx.spawn(async move |this, app| {
            let result = task.await;
            let _ = this.update(app, |this, cx| {
                this.save_running = false;
                this.busy = this.preview_running || this.save_running;
                match result {
                    Ok(()) => {
                        this.status = format!("已保存：{}", dest.display());
                        this.error = None;
                    }
                    Err(e) => {
                        this.status = "另存为失败".to_string();
                        this.error = Some(format!("{e:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn tool_button(
        &self,
        label: &'static str,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Button {
        let this = cx.entity();
        Button::new(label)
            .outline()
            .label(label)
            .on_click(move |_ev, _window, cx| {
                let _ = this.update(cx, |this, cx| f(this, cx));
            })
    }
}

impl Render for PhotoEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let titlebar = window_titlebar(format!("原图编辑 · {}", self.label), "imageflow 本地处理");

        let image_path = self.preview.clone();
        let stage = if image_path.is_file() {
            gpui::img(image_path)
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element()
        } else {
            gpui::div()
                .id("editor-missing-source")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(
                    gpui::div()
                        .text_size(px(28.))
                        .text_color(col(BORDER_LIGHT))
                        .child("▧"),
                )
                .child(
                    gpui::div()
                        .text_size(px(13.))
                        .text_color(col(TEXT_SECONDARY))
                        .child("原图文件不可用"),
                )
                .into_any_element()
        };

        let entity = cx.entity();
        let tools = gpui::div()
            .id("editor-tools")
            .w(px(280.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(16.))
            .border_l_1()
            .border_color(col(BORDER_LIGHT))
            .child(section("旋转与翻转"))
            .child(row("editor-row-rotate", vec![
                self.tool_button("左转 90°", cx, |this, cx| {
                    this.mutate(cx, |o| o.rotate_degrees = (o.rotate_degrees + 270) % 360)
                }),
                self.tool_button("右转 90°", cx, |this, cx| {
                    this.mutate(cx, |o| o.rotate_degrees = (o.rotate_degrees + 90) % 360)
                }),
                self.tool_button("180°", cx, |this, cx| {
                    this.mutate(cx, |o| o.rotate_degrees = (o.rotate_degrees + 180) % 360)
                }),
            ]))
            .child(row("editor-row-flip", vec![
                self.tool_button("左右翻转", cx, |this, cx| {
                    this.mutate(cx, |o| o.flip_h = !o.flip_h)
                }),
                self.tool_button("上下翻转", cx, |this, cx| {
                    this.mutate(cx, |o| o.flip_v = !o.flip_v)
                }),
            ]))
            .child(section("颜色滤镜"))
            .child(row("editor-row-filter", vec![
                self.tool_button("原色", cx, |this, cx| {
                    this.mutate(cx, |o| o.filter = ColorFilter::None)
                }),
                self.tool_button("灰度", cx, |this, cx| {
                    this.mutate(cx, |o| o.filter = ColorFilter::Grayscale)
                }),
                self.tool_button("棕褐", cx, |this, cx| {
                    this.mutate(cx, |o| o.filter = ColorFilter::Sepia)
                }),
                self.tool_button("反色", cx, |this, cx| {
                    this.mutate(cx, |o| o.filter = ColorFilter::Invert)
                }),
            ]))
            .child(section("亮度 · 对比度 · 饱和度"))
            .child(hint("每点一次按 10% 递增，范围 50%–200%"))
            .child(pair("brightness", "brightness-minus", "brightness-plus", "亮度", self.options.brightness, entity.clone(), |o, d| {
                o.brightness = clamp_step(o.brightness, d)
            }))
            .child(pair("contrast", "contrast-minus", "contrast-plus", "对比度", self.options.contrast, entity.clone(), |o, d| {
                o.contrast = clamp_step(o.contrast, d)
            }))
            .child(pair("saturation", "saturation-minus", "saturation-plus", "饱和度", self.options.saturation, entity.clone(), |o, d| {
                o.saturation = clamp_step(o.saturation, d)
            }))
            .child(section("输出"))
            .child({
                let photo = self.photo.clone();
                Button::new("editor-info")
                    .label("查看图像信息")
                    .on_click(move |_ev, window, cx| {
                        crate::ui::app::open_photo_dialog(photo.clone(), window, cx)
                    })
            })
            .child(
                self.tool_button("重置全部编辑", cx, |this, cx| {
                    this.options = EditOptions::default();
                    this.schedule_preview(cx);
                }),
            )
            .child({
                let this = entity.clone();
                let label = self.label.clone();
                let save = Button::new("editor-save")
                    .label("另存为 PNG（原分辨率）")
                    .on_click(move |_ev, _window, cx| {
                        let default_name = PathBuf::from(&label)
                            .file_stem()
                            .map(|s| format!("{}-edited.png", s.to_string_lossy()))
                            .unwrap_or_else(|| "edited.png".to_string());
                        // rfd 必须在 update 之外调用，避免窗口事件回调重入。
                        if let Some(dest) = rfd::FileDialog::new()
                            .set_title("另存为编辑结果")
                            .set_file_name(default_name)
                            .add_filter("PNG 图片", &["png"])
                            .save_file()
                        {
                            let _ = this.update(cx, |this, cx| this.save_as(cx, dest));
                        }
                    });
                gpui::div()
                    .id("editor-save")
                    .mt(px(6.))
                    .child(save)
            })
            .child({
                let error = self.error.clone().unwrap_or_default();
                if error.is_empty() {
                    gpui::div().id("editor-no-error").into_any_element()
                } else {
                    gpui::div()
                        .id("editor-error")
                        .mt(px(8.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded_md()
                        .bg(gpui::Hsla::from(gpui::rgb(DANGER)).opacity(0.1))
                        .text_size(px(11.))
                        .text_color(col(DANGER))
                        .child(error)
                        .into_any_element()
                }
            });

        let footer = gpui::div()
            .id("editor-footer")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(8.))
            .border_t_1()
            .border_color(col(BORDER_LIGHT))
            .text_size(px(12.))
            .text_color(col(TEXT_SECONDARY))
            .child(if self.busy { "● 处理中 …" } else { "○ 就绪" })
            .child(self.status.clone());

        gpui::div()
            .id("photo-editor")
            .size_full()
            .flex()
            .flex_col()
            .bg(col(BG_WHITE))
            .child(titlebar)
            .child(
                gpui::div()
                    .id("editor-body")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(
                        gpui::div()
                            .id("editor-preview")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .bg(col(BG_WARM))
                            .p(px(12.))
                            .child(
                                gpui::div()
                                    .id("editor-stage")
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(stage),
                            ),
                    )
                    .child(tools),
            )
            .child(footer)
    }
}

fn clamp_step(value: f32, delta: f32) -> f32 {
    (value + delta).clamp(0.5, 2.0)
}

fn section(text: &'static str) -> gpui::AnyElement {
    gpui::div()
        .id(text)
        .mt(px(4.))
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(col(GOLD))
        .child(text)
        .into_any_element()
}

fn hint(text: &'static str) -> gpui::AnyElement {
    gpui::div()
        .id("editor-hint")
        .text_size(px(10.))
        .text_color(col(TEXT_SECONDARY))
        .child(text)
        .into_any_element()
}

fn row(id: &'static str, children: Vec<Button>) -> gpui::AnyElement {
    gpui::div()
        .id(id)
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(6.))
        .children(children)
        .into_any_element()
}

fn pair(
    id: &'static str,
    minus_id: &'static str,
    plus_id: &'static str,
    label: &'static str,
    value: f32,
    this: Entity<PhotoEditor>,
    apply_delta: fn(&mut EditOptions, f32),
) -> gpui::AnyElement {
    let minus = this.clone();
    let minus = Button::new(minus_id)
        .label("−")
        .on_click(move |_ev, _w, cx| {
            let _ = minus.update(cx, |this, cx| {
                this.mutate(cx, |o| apply_delta(o, -0.1))
            });
        });
    let plus = Button::new(plus_id)
        .label("+")
        .on_click(move |_ev, _w, cx| {
            let _ = this.update(cx, |this, cx| {
                this.mutate(cx, |o| apply_delta(o, 0.1))
            });
        });
    gpui::div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .child(
            gpui::div()
                .w(px(64.))
                .text_size(px(12.))
                .text_color(col(TEXT_PRIMARY))
                .child(label),
        )
        .child(minus)
        .child(
            gpui::div()
                .w(px(44.))
                .text_size(px(12.))
                .text_color(col(TEXT_PRIMARY))
                .child(format!("{:.0}%", value * 100.0)),
        )
        .child(plus)
        .into_any_element()
}

/// 从主窗口点击照片处调用：打开独立浮窗展示快速预览并编辑。
pub fn open_photo_editor(photo: Photo, paths: Paths, cx: &mut App) {
    let options = WindowOptions {
        kind: WindowKind::Floating,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::new(px(160.), px(90.)),
            size: Size::new(px(1100.), px(760.)),
        })),
        window_min_size: Some(Size::new(px(880.), px(600.))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some(SharedString::from("Akashic 原图编辑")),
            appears_transparent: true,
            traffic_light_position: if cfg!(target_os = "macos") {
                Some(gpui::point(px(14.), px(16.)))
            } else {
                None
            },
        }),
        ..Default::default()
    };
    if let Err(e) = cx.open_window(options, |window, cx| {
        let editor = cx.new(|_| PhotoEditor::new(photo, paths.clone()));
        editor.update(cx, |editor, cx| editor.start(cx));
        cx.new(|cx| Root::new(editor, window, cx))
    }) {
        eprintln!("[akashic] 打开原图编辑窗口失败: {e}");
    }
}
