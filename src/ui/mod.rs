//! UI 层：只依赖 core 与 gpui，不直接访问文件系统（M1 起由 core 提供数据）。

pub mod analyze;
pub mod app;
pub mod assets;
pub mod editor;
pub mod library;
pub mod ops;

use gpui_component::InteractiveElementExt as _;

use gpui::{
    AnyElement, SharedString, WindowControlArea, div, prelude::*, px,
};

const TITLEBAR_BG: u32 = 0x1A2634;
const TITLE_SUB: u32 = 0x8A9AAA;
const TITLE_GOLD: u32 = 0xC9A96E;

fn title_color(hex: u32) -> gpui::Hsla {
    gpui::Hsla::from(gpui::rgb(hex))
}

/// 单个红黄绿窗口按钮。
///
/// Windows 上同时设置 `window_control_area`：系统命中测试会把该区域识别成
/// HTCLOSE / HTMINBUTTON / HTMAXBUTTON 并触发真正的窗口动作；`on_click` 是
/// Linux/其他平台以及系统命中测试尚未更新时的兜底（两者互斥，不会重复触发）。
fn traffic_button(
    id: &'static str,
    color: u32,
    area: WindowControlArea,
    action: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .size(px(13.))
        .rounded_full()
        .bg(title_color(color))
        .window_control_area(area)
        .on_click(move |_ev, window, cx| action(window, cx))
        .into_any_element()
}

fn window_controls() -> AnyElement {
    div()
        .id("window-controls")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .child(traffic_button("win-close", 0xE0544C, WindowControlArea::Close, |w, _| {
            w.remove_window()
        }))
        .child(traffic_button("win-min", 0xE5B84C, WindowControlArea::Min, |w, _| {
            w.minimize_window()
        }))
        .child(traffic_button("win-max", 0x5CB85C, WindowControlArea::Max, |w, _| {
            w.zoom_window()
        }))
        .into_any_element()
}

/// 自绘标题栏：系统默认标题栏由 `main.rs` 的 `TitlebarOptions::appears_transparent` 隐藏。
///
/// macOS 使用系统原生红绿灯（标题栏左侧留白），Windows/Linux 使用上面的自绘按钮。
/// 标题区域是拖拽区（`WindowControlArea::Drag`），双击标题可最大化/还原。
pub fn window_titlebar(
    title: impl Into<SharedString>,
    badge: impl Into<SharedString>,
) -> AnyElement {
    let left = if cfg!(target_os = "macos") {
        div().id("mac-traffic-space").w(px(80.)).h(px(13.)).into_any_element()
    } else {
        window_controls()
    };
    div()
        .id("titlebar")
        .h(px(48.))
        .bg(title_color(TITLEBAR_BG))
        .flex()
        .flex_row()
        .items_center()
        .px(px(20.))
        .gap(px(16.))
        .child(left)
        .child(
            div()
                .id("app-title")
                .flex_1()
                .min_w_0()
                .window_control_area(WindowControlArea::Drag)
                .on_double_click(|_ev, window, _cx| {
                    if cfg!(target_os = "macos") {
                        window.titlebar_double_click();
                    } else {
                        window.zoom_window();
                    }
                })
                .text_color(gpui::white())
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .child(
            div()
                .id("status-tag")
                .text_size(px(11.))
                .text_color(title_color(TITLE_SUB))
                .bg(gpui::Hsla::from(gpui::rgb(0xFFFFFF)).opacity(0.06))
                .px(px(14.))
                .py(px(3.))
                .rounded_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .id("badge-dot")
                        .size(px(6.))
                        .rounded_full()
                        .bg(title_color(TITLE_GOLD)),
                )
                .child(badge.into()),
        )
        .into_any_element()
}
