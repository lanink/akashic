//! 组合根：Application / Assets / GPUI 初始化 / 打开窗口。
//! 页面渲染与状态在 `akashic::ui`，纯数据模型在 `akashic::core`。

use gpui::*;
use gpui_component::Root;

use akashic::core::config::Paths;
use akashic::ui::app::{new_app_state, AppRoot};
use akashic::ui::assets::Assets;

fn main() {
    // 确保本机应用数据目录就绪（索引、缓存、回收站）。
    if let Some(paths) = Paths::platform_default() {
        if let Err(e) = paths.ensure_dirs() {
            eprintln!("[akashic] 无法初始化本地目录: {e}");
        }
    }

    let app = gpui::Application::new().with_assets(Assets);
    app.run(move |cx| {
        gpui_component::init(cx);
        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::new(px(80.), px(40.)),
                    size: Size::new(px(1200.), px(780.)),
                })),
                ..WindowOptions::default()
            };
            cx.open_window(options, |window, cx| {
                let state = new_app_state(cx);
                let view = cx.new(|_| AppRoot::new(state));
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
