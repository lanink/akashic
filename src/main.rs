//! 组合根：Application / Assets / GPUI 初始化 / 打开窗口。
//! 页面渲染与状态在 `akashic::ui`，纯数据模型在 `akashic::core`。

use gpui::*;
use gpui_component::Root;

use akashic::core::config::Paths;
use akashic::ui::app::{new_app_state, AppRoot};
use akashic::ui::assets::Assets;

fn main() {
    // 确保本机应用数据目录就绪（索引、缓存、回收站）。
    // 系统应用数据目录不可用时回退到用户主目录下的持久目录；
    // 临时目录会被系统清理，不能用来存放索引库和回收站里的原文件。
    let paths = Paths::platform_default().unwrap_or_else(|| {
        dirs::home_dir()
            .map(|h| Paths::from_app_dir(h.join(".akashic")))
            .unwrap_or_else(|| Paths::from_app_dir(std::env::temp_dir().join("akashic")))
    });
    if let Err(e) = paths.ensure_dirs() {
        // 目录建不起来时 db::open 必然失败，继续启动只会得到一个所有操作静默失败的界面。
        eprintln!("[akashic] 无法初始化本地目录，应用无法启动: {e}");
        return;
    }

    let app = gpui::Application::new().with_assets(Assets);
    app.run(move |cx| {
        gpui_component::init(cx);
        cx.spawn(async move |cx| {
            // 1200×780 仅作为首次打开时的初始尺寸，不再与 UI 布局强绑定；
            // 窗口可自由缩放，最小尺寸保证侧栏 + 内容区不挤压。
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::new(px(80.), px(40.)),
                    size: Size::new(px(1200.), px(780.)),
                })),
                window_min_size: Some(Size::new(px(960.), px(640.))),
                ..WindowOptions::default()
            };
            let paths_for_window = paths.clone();
            if let Err(e) = cx.open_window(options, |window, cx| {
                let state = new_app_state(cx, paths_for_window);
                let view = cx.new(|_| AppRoot::new(state));
                cx.new(|cx| Root::new(view, window, cx))
            }) {
                // detach() 会丢弃 Result，不打印就只剩一个无窗口的空事件循环。
                eprintln!("[akashic] 创建窗口失败: {e}");
            }
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
