//! 原图编辑浮窗：点击照片后以独立窗口展示原图，并用 imageflow 做本地编辑。
//!
//! 编辑能力：旋转 / 翻转 / 灰度·棕褐·反色 / 亮度·对比度·饱和度 / 等比缩放。
//! 预览在后台执行器运行，不阻塞主窗口；「另存为」按原分辨率输出 PNG。

use std::path::PathBuf;

use gpui::*;
use gpui_component::Root;
use gpui_component::button::Button;

use crate::core::config::Paths;
use crate::core::edit::{ColorFilter, EditOptions, apply, preview_path};
use crate::core::model::Photo;
use crate::ui::window_titlebar;

const BG_WHITE: u32 = 0xFFFFFF;
const BG_WARM: u32 = 0xF5F2ED;
const TEXT_PRIMARY: u32 = 0x1A2634;
const TEXT_SECONDARY: u32 = 0x5A6A7A;
const BORDER_LIGHT: u32 = 0xE8E4DE;
const GOLD: u32 = 0xC9A96E;
const DANGER: u32 = 0xE0544C;

fn col(hex: u32) -> gpui::Hsla {
    gpui::Hsla::from(gpui::rgb(hex))
}

pub struct PhotoEditor {
    photo: Photo,
    source: PathBuf,
    paths: Paths,
    label: String,
    /// 当前展示的图片路径：未编辑时是原图，编辑成功后指向缓存的 PNG。
    preview: PathBuf,
    options: EditOptions,
    status: String,
    error: Option<String>,
    busy: bool,
    /// 后台任务代次：旧任务返回时若代次已变则丢弃，避免慢任务覆盖新预览。
    generation: u64,
}

impl PhotoEditor {
    fn new(photo: Photo, paths: Paths) -> Self {
        let source = photo.path.clone();
        let label = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名照片".to_string());
        let status = if source.is_file() {
            "原图 · 未编辑".to_string()
        } else {
            "原图文件已移动或删除，编辑不可用".to_string()
        };
        Self {
            preview: source.clone(),
            photo,
            source,
            paths,
            label,
            options: EditOptions {
                max_dimension: 1600,
                ..Default::default()
            },
            status,
            error: None,
            busy: false,
            generation: 0,
        }
    }

    fn preview_dir(&self) -> PathBuf {
        self.paths.data_dir.join("editor")
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
        self.status = "正在用 imageflow 生成预览 …".to_string();
        self.error = None;

        let src = self.source.clone();
        let dest = preview_path(&self.preview_dir(), &self.source, &self.options);
        let task_dest = dest.clone();
        let options = self.options;
        let task = cx.background_executor().spawn(async move { apply(&src, &task_dest, &options) });
        cx.spawn(async move |this, app| {
            let result = task.await;
            let _ = this.update(app, |this, cx| {
                // 只有最新一次请求可以提交结果。
                if this.generation != generation {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(()) => {
                        this.preview = dest;
                        this.status = if this.options.has_user_edits() {
                            "已应用编辑 · 预览最长边 1600px".to_string()
                        } else {
                            "原图 · 未编辑（预览最长边 1600px）".to_string()
                        };
                        this.error = None;
                    }
                    Err(e) => {
                        this.error = Some(format!("编辑失败：{e:#}"));
                        this.status = "预览失败，仍显示上一张图片".to_string();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save_as(&mut self, cx: &mut Context<Self>, dest: PathBuf) {
        if !self.source.is_file() {
            self.error = Some("原图文件不存在，无法另存为".to_string());
            cx.notify();
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.busy = true;
        self.status = format!("正在保存原分辨率 PNG 到 {} …", dest.display());
        self.error = None;

        let src = self.source.clone();
        let mut options = self.options;
        options.max_dimension = 0;
        let staged = preview_path(&self.preview_dir(), &self.source, &options);
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
                if this.generation != generation {
                    return;
                }
                this.busy = false;
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
                    this.options = EditOptions {
                        max_dimension: 1600,
                        ..Default::default()
                    };
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

/// 从主窗口点击照片处调用：打开独立浮窗展示原图并编辑。
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
        cx.new(|cx| Root::new(editor, window, cx))
    }) {
        eprintln!("[akashic] 打开原图编辑窗口失败: {e}");
    }
}
