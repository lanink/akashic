use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::dialog::Dialog;
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::WindowExt;

use crate::core::config::Paths;
use crate::core::model::{ScanProgress, ScanStats};
use crate::ui::library::LibraryData;

// ===== 设计稿配色（akashic-ds.html） =====
const PRIMARY: u32 = 0x1A2634; // 深蓝 主色
const PRIMARY_LIGHTER: u32 = 0x3A506A;
const GOLD: u32 = 0xC9A96E; // 暗金 关键操作/选中
const GOLD_PALE: u32 = 0xF5EDDA;
const BG_WARM: u32 = 0xF5F2ED; // 内容区暖白
const BG_WHITE: u32 = 0xFFFFFF;
const BG_SIDEBAR: u32 = 0x1A2634;
const TEXT_PRIMARY: u32 = 0x1A2634;
const TEXT_SECONDARY: u32 = 0x5A6A7A;
const TEXT_LIGHT: u32 = 0x8A9AAA;
const BORDER_LIGHT: u32 = 0xE8E4DE;
const DANGER: u32 = 0xE0544C;

/// hex -> Hsla
fn col(hex: u32) -> Hsla {
    Hsla::from(gpui::rgb(hex))
}

/// rgba(0-255 各分量, 透明度 0..1) -> Hsla
fn rgba(r: u32, g: u32, b: u32, a: f32) -> Hsla {
    Hsla::from(gpui::rgb((r << 16) | (g << 8) | b)).opacity(a)
}

// ===== 页面 =====
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Organize,
    Library,
    Rename,
    Trash,
}

// ===== 照片缩略图配色 =====
fn photo_block_bg(hue: f32) -> Hsla {
    gpui::hsla(hue / 360.0, 0.36, 0.74, 1.0)
}
fn photo_icon_bg(hue: f32) -> Hsla {
    gpui::hsla(hue / 360.0, 0.46, 0.42, 1.0)
}

// ===== 应用全局状态 =====
pub struct AppState {
    page: Page,
    /// 本机路径配置
    paths: Paths,
    /// 相册库数据快照（titlebar/首页统计也从这里取）
    data: LibraryData,
    active_folder_id: Option<String>,
    folder_expanded: HashSet<String>, // 展开的目录 id 集合
    selected: HashSet<usize>,
    /// 扫描状态文字
    scan_status: String,
    is_scanning: bool,
    /// 后台扫描进度接收端（Send，可放 Entity）
    progress_rx: Option<Receiver<ScanProgress>>,
}

impl AppState {
    fn new(cx: &mut App, paths: Paths) -> Entity<Self> {
        cx.new(|_| {
            // 启动时尝试从已有索引加载；失败则用空快照（不影响 UI 启动）
            let db_path = paths.db_path.clone();
            let data = match crate::core::db::open(&db_path) {
                Ok(conn) => LibraryData::reload(&conn).unwrap_or_default(),
                Err(_) => LibraryData::default(),
            };
            Self {
                page: Page::Library,
                paths,
                data,
                active_folder_id: None,
                folder_expanded: HashSet::new(),
                selected: HashSet::new(),
                scan_status: String::new(),
                is_scanning: false,
                progress_rx: None,
            }
        })
    }

    /// 当前可见照片数量（供 UI 显示）。
    pub fn visible_count(&self) -> usize {
        self.data.visible_photos().len()
    }

    /// 启动后台扫描：选择目录 → 后台线程扫描写库 → 进度回传 UI。
    pub fn start_scan(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if self.is_scanning {
            return;
        }
        self.is_scanning = true;
        self.scan_status = format!("正在扫描 {} …", root.display());

        let (tx, rx) = std::sync::mpsc::channel::<ScanProgress>();
        self.progress_rx = Some(rx);

        let paths = self.paths.clone();

        // 后台线程：自己打开 DB 连接执行扫描（Connection 不 Send，不能跨线程共享）
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<ScanStats> {
                let conn = crate::core::db::open(&paths.db_path)?;
                crate::core::scanner::scan_root(&paths, &conn, &root, Some(&tx))
            })();
            if let Err(e) = result {
                let _ = tx.send(ScanProgress::Failed { message: e.to_string() });
            }
        });

        // UI 侧：定时轮询进度通道，收到 Finished/Failed 后刷新数据快照
        cx.spawn(async move |entity, app: &mut AsyncApp| {
            loop {
                // 每 150ms 轮询一次，避免占满 UI 主线程
                gpui::background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;

                // 从实体里取出 receiver 做一次非阻塞接收
                let got = entity
                    .update(app, |this: &mut Self, _cx| {
                        if let Some(rx) = &this.progress_rx {
                            match rx.try_recv() {
                                Ok(p) => Some(p),
                                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    Some(ScanProgress::Failed {
                                        message: "扫描线程意外退出".to_string(),
                                    })
                                }
                            }
                        } else {
                            None
                        }
                    })
                    .ok()
                    .flatten();

                if let Some(p) = got {
                    let mut done = false;
                    entity
                        .update(app, |this: &mut Self, cx| {
                            match p {
                                ScanProgress::Started { root } => {
                                    this.scan_status = format!("正在扫描 {} …", root.display());
                                }
                                ScanProgress::Scanning { processed } => {
                                    this.scan_status = format!("正在扫描 … 已处理 {} 张", processed);
                                }
                                ScanProgress::Finished { stats } => {
                                    if let Ok(conn) = crate::core::db::open(&this.paths.db_path) {
                                        this.data = LibraryData::reload(&conn).unwrap_or_default();
                                    }
                                    this.scan_status = format!(
                                        "扫描完成：新增 {} 张，跳过 {} 张，清理 {} 张，耗时 {} ms",
                                        stats.photos_added, stats.photos_skipped,
                                        stats.photos_removed, stats.duration_ms
                                    );
                                    this.is_scanning = false;
                                    this.progress_rx = None;
                                    done = true;
                                }
                                ScanProgress::Failed { message } => {
                                    this.scan_status = format!("扫描失败：{message}");
                                    this.is_scanning = false;
                                    this.progress_rx = None;
                                    done = true;
                                }
                            }
                            cx.notify();
                        })
                        .ok();
                    if done {
                        break;
                    }
                }
            }
        }).detach();
    }
}

// ===== 根组件 =====
pub struct AppRoot {
    app: Entity<AppState>,
}

impl AppRoot {
    pub fn new(app: Entity<AppState>) -> Self {
        Self { app }
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.app.read(cx);

        let titlebar = div()
            .id("titlebar")
            .h(px(48.))
            .bg(col(BG_SIDEBAR))
            .flex().flex_row()
            .items_center()
            .px(px(20.))
            .gap(px(16.))
            .child(
                div().flex().flex_row().gap(px(8.)).children(vec![
                    dot(DANGER),
                    dot(0xE5B84C),
                    dot(0x5CB85C),
                ]),
            )
            .child(
                div()
                    .id("app-title")
                    .ml(px(8.))
                    .flex_1()
                    .text_color(gpui::white())
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Akashic · 相册整理师"),
            )
            .child(
                div()
                    .id("status-tag")
                    .text_size(px(11.))
                    .text_color(col(TEXT_LIGHT))
                    .bg(rgba(255, 255, 255, 0.06))
                    .px(px(14.))
                    .py(px(3.))
                    .rounded_full()
                    .flex().flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child(Icon::new(IconName::CircleCheck).size_3().text_color(col(GOLD)))
                    .child("本地 · 离线"),
            );

        let sidebar = self.sidebar(state);
        let content = div()
            .id("content")
            .flex_1()
            .min_h_0()
            .bg(col(BG_WARM))
            .px(px(24.))
            .py(px(20.))
            .overflow_y_scroll()
            .child(self.render_page(state));

        div()
            .id("window")
            .flex_1()
            .size_full()
            .items_center()
            .justify_center()
            .bg(col(0xE8E4DE))
            .child(
                div()
                    .id("windowcard")
                    .w(px(1200.))
                    .h(px(780.))
                    .max_h_full()
                    .flex().flex_col()
                    .overflow_hidden()
                    .rounded_lg()
                    .bg(col(BG_WHITE))
                    .shadow_sm()
                    .child(titlebar)
                    .child(
                        div()
                            .id("mainlayout")
                            .flex_1()
                            .min_h_0()
                            .flex().flex_row()
                            .child(sidebar)
                            .child(content),
                    ),
            )
    }
}

/// 供组合根创建初始状态（main.rs 使用）。
pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState> {
    AppState::new(cx, paths)
}

fn dot(color: u32) -> impl IntoElement + 'static {
    div().id(("dot", color)).size(px(13.)).rounded_full().bg(col(color))
}

// ===== 工具/卡片辅助 =====
fn page_title(main: impl Into<SharedString>, sub: impl Into<SharedString>) -> impl IntoElement {
    div()
        .id("page-title")
        .flex().flex_row()
        .items_baseline()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::BOLD)
                .text_color(col(TEXT_PRIMARY))
                .child(main.into()),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(col(TEXT_SECONDARY))
                .child(sub.into()),
        )
}

impl AppRoot {
    /// 深色侧边导航栏
    fn sidebar(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        let nav = |page: Page, icon: IconName, label: &'static str, badge: Option<&'static str>| {
            let active = state.page == page;
            let app = app.clone();
            let btn = Button::new(label)
                .ghost()
                .w_full()
                .justify_start()
                .px(px(16.))
                .py(px(10.))
                .gap(px(12.))
                .rounded_md()
                .border_l_3()
                .icon(icon)
                .label(label)
                .when(active, |b| {
                    b.bg(rgba(201, 169, 110, 0.08)).text_color(col(GOLD)).border_color(col(GOLD))
                })
                .when(!active, |b| {
                    b.border_color(transparent_black()).text_color(rgba(255, 255, 255, 0.5))
                })
                .when_some(badge, |b, badge_text| {
                    b.children(vec![
                        div()
                            .ml_auto()
                            .text_size(px(10.))
                            .px(px(10.))
                            .py(px(1.))
                            .rounded_full()
                            .when(active, |e| e.bg(rgba(201, 169, 110, 0.2)).text_color(col(GOLD)))
                            .when(!active, |e| e.bg(rgba(255, 255, 255, 0.08)).text_color(rgba(255, 255, 255, 0.5)))
                            .child(badge_text),
                    ])
                })
                .on_click(move |_ev, _w, cx| {
                    let s = app.update(cx, |s, cx| {
                        s.page = page;
                        cx.notify();
                    });
                    let _ = s;
                });
            btn.into_any_element()
        };

        let setting_btn = Button::new("nav-settings")
            .ghost()
            .w_full()
            .justify_start()
            .px(px(16.))
            .py(px(10.))
            .gap(px(12.))
            .rounded_md()
            .border_l_3()
            .border_color(transparent_black())
            .icon(IconName::Settings)
            .label("设置")
            .text_color(rgba(255, 255, 255, 0.5))
            .on_click(|_ev, _w, _cx| {})
            .into_any_element();

        div()
            .id("sidebar")
            .w(px(200.))
            .flex_none()
            .min_h_0()
            .flex().flex_col()
            .bg(col(BG_SIDEBAR))
            .pt(px(4.))
            .child(nav(Page::Home, IconName::LayoutDashboard, "首页", None))
            .child(nav(Page::Organize, IconName::Settings2, "智能整理", Some("12")))
            .child(nav(Page::Library, IconName::GalleryVerticalEnd, "相册库", Some("1,247")))
            .child(nav(Page::Rename, IconName::CaseSensitive, "重命名", None))
            .child(nav(Page::Trash, IconName::Delete, "回收站", Some("4")))
            .child(div().id("sidebar-divider").h(px(1.)).my(px(12.)).mx(px(20.)).bg(rgba(255, 255, 255, 0.06)))
            .child(setting_btn)
            .child(
                div()
                    .id("sidebar-footer")
                    .flex_1()
                    .mt_auto()
                    .px(px(20.))
                    .pb(px(16.))
                    .child(
                        div()
                            .id("privacy-badge")
                            .text_size(px(10.))
                            .text_color(rgba(255, 255, 255, 0.25))
                            .flex().flex_row()
                            .items_center()
                            .gap(px(6.))
                            .child(Icon::new(IconName::CircleUser).size_3_5().text_color(rgba(255, 255, 255, 0.25)))
                            .child("100% 本地 · 零上传"),
                    ),
            )
    }

    fn render_page(&self, state: &AppState) -> impl IntoElement + 'static {
        match state.page {
            Page::Home => self.page_home(state).into_any_element(),
            Page::Organize => self.page_organize().into_any_element(),
            Page::Library => self.page_library(state).into_any_element(),
            Page::Rename => self
                .page_placeholder(IconName::CaseSensitive, "重命名功能 — 从相册库选择照片后使用")
                .into_any_element(),
            Page::Trash => self
                .page_placeholder(IconName::Delete, "回收站 — 删除的照片会保存在这里 30 天")
                .into_any_element(),
        }
    }
}

impl AppRoot {
    // ===== 首页 =====
    fn page_home(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        // 一次性提取真实统计到 owned 值，避免长时间持有 state 引用造成借用冲突
        let total = state.data.total_count;
        let total_bytes = state.data.total_bytes;
        let root_count = state.data.root_count;
        let size_text = if total_bytes >= 1024 * 1024 * 1024 {
            format!("{:.1} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if total_bytes >= 1024 * 1024 {
            format!("{:.1} MB", total_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{} KB", total_bytes / 1024)
        };
        let stats: Vec<(String, &'static str, u32)> = vec![
            (format!("{}", total), "总照片", PRIMARY),
            (size_text.clone(), "占用空间", PRIMARY),
            (format!("{}", root_count), "扫描目录", GOLD),
            (format!("{}", state.data.visible_photos().len()), "当前可见", PRIMARY),
        ];
        let stat_cards = stats.into_iter().enumerate().map(|(si, (v, label, color))| {
            div()
                .id(("stat", si))
                .flex_1()
                .bg(col(BG_WHITE))
                .rounded_lg()
                .px(px(20.))
                .py(px(18.))
                .shadow_sm()
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(col(color))
                        .child(v.clone()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .mt(px(2.))
                        .text_color(col(TEXT_SECONDARY))
                        .child(label.to_owned()),
                )
        });

        let records: Vec<(&'static str, String)> = if state.data.loaded {
            vec![
                ("索引状态", format!("已建立 · {} 个目录", root_count)),
                ("照片总数", format!("{} 张", total)),
                ("占用空间", size_text.clone()),
                ("最近扫描", if root_count > 0 { "索引已就绪".to_string() } else { "尚未扫描".to_string() }),
            ]
        } else {
            vec![
                ("索引状态", "尚未建立".to_string()),
                ("提示", "前往相册库选择目录".to_string()),
                ("整理工具", "扫描后自动解锁".to_string()),
                ("隐私", "100% 本地 · 零上传".to_string()),
            ]
        };
        let record_rows = records.into_iter().enumerate().map(|(ri, (a, b))| {
            div()
                .id(("record", ri))
                .flex_1()
                .flex().flex_row()
                .justify_between()
                .px(px(14.))
                .py(px(10.))
                .bg(col(BG_WARM))
                .rounded_md()
                .text_size(px(13.))
                .child(div().text_color(col(TEXT_SECONDARY)).child(a))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(b))
        });

        let app2 = app.clone();
        let organize_btn = Button::new("home-organize")
            .primary()
            .icon(IconName::Settings2)
            .label("开始整理")
            .on_click(move |_ev, _w, cx| {
                let s = app2.update(cx, |s, cx| {
                    s.page = Page::Organize;
                    cx.notify();
                });
                let _ = s;
            });

        div()
            .id("page-home")
            .flex().flex_col()
            .child(
                div()
                    .id("home-header")
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .mb(px(16.))
                    .child(page_title("📊 相册概览", subtitle_for_home(state)))
                    .child(organize_btn),
            )
            .child(
                div()
                    .id("home-stats")
                    .flex().flex_row()
                    .gap(px(14.))
                    .children(stat_cards.collect::<Vec<_>>()),
            )
            .child(
                div()
                    .id("home-records")
                    .flex_1()
                    .bg(col(BG_WHITE))
                    .rounded_lg()
                    .px(px(20.))
                    .py(px(18.))
                    .shadow_sm()
                    .mt(px(18.))
                    .child(
                        div()
                            .id("records-label")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(TEXT_SECONDARY))
                            .mb(px(10.))
                            .child("📋 最近整理记录"),
                    )
                    .child(
                        div()
                            .id("records-grid")
                            .grid()
                            .grid_cols(2)
                            .gap(px(8.))
                            .children(record_rows.collect::<Vec<_>>()),
                    ),
            )
    }

    // ===== 智能整理页 =====
    fn page_organize(&self) -> impl IntoElement + 'static {
        let tools = vec![
            (IconName::Copy, "相似照片", "连拍 & 重复角度", "12组", false),
            (IconName::EyeOff, "模糊检测", "手抖 / 失焦", "43张", false),
            (IconName::Frame, "截图筛选", "屏幕录制 · 截屏", "218张", false),
            (IconName::File, "大文件", "按大小排序", "34个", false),
            (IconName::CaseSensitive, "智能重命名", "统一命名风格", "⭐", true),
            (IconName::FolderOpen, "智能归档", "自动归入目录", "⭐", true),
        ];
        let tool_cards = tools.into_iter().enumerate().map(|(ti, (icon, name, desc, badge, star))| {
            div()
                .id(("tool", ti))
                .flex_1()
                .bg(col(BG_WHITE))
                .rounded_lg()
                .px(px(16.))
                .py(px(16.))
                .shadow_sm()
                .cursor_pointer()
                .flex().flex_row()
                .items_center()
                .gap(px(14.))
                .child(Icon::new(icon).size(px(24.)).text_color(col(PRIMARY_LIGHTER)))
                .child(
                    div()
                        .id(("toolinfo", ti))
                        .flex_1()
                        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(name.to_owned()))
                        .child(
                            div()
                                .text_size(px(12.))
                                .mt(px(2.))
                                .text_color(col(TEXT_SECONDARY))
                                .child(desc.to_owned()),
                        ),
                )
                .child(
                    div()
                        .id(("toolbadge", ti))
                        .ml_auto()
                        .text_size(px(11.))
                        .px(px(14.))
                        .py(px(2.))
                        .rounded_full()
                        .font_weight(FontWeight::SEMIBOLD)
                        .when(star, |d| d.bg(col(GOLD)).text_color(gpui::white()))
                        .when(!star, |d| d.bg(col(GOLD_PALE)).text_color(col(PRIMARY)))
                        .child(badge.to_owned()),
                )
        });

        let stories = vec![
            ("JP", "日本旅行", "85 张 · 2024.08"),
            ("🎂", "生日派对", "34 张 · 2024.07"),
            ("⛰", "四姑娘山", "47 张 · 2024.06"),
            ("🏖", "三亚旅行", "203 张 · 2023.12"),
            ("🎄", "圣诞聚餐", "28 张 · 2023.12"),
        ];
        // 故事标签用文本 + 品牌色块，避免 emoji 渲染不确定
        let story_cards = stories.into_iter().enumerate().map(|(i, (mark, name, count))| {
            div()
                .id(("story", i))
                .min_w(px(140.))
                .bg(col(BG_WHITE))
                .rounded_lg()
                .px(px(14.))
                .py(px(14.))
                .shadow_sm()
                .cursor_pointer()
                .child(
                    div()
                        .size(px(40.))
                        .rounded_full()
                        .bg([rgba(201, 169, 110, 0.25), rgba(58, 80, 106, 0.25)][i % 2])
                        .text_color(col(PRIMARY_LIGHTER))
                        .items_center()
                        .justify_center()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .child(mark.to_owned()),
                )
                .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).mt(px(8.)).child(name.to_owned()))
                .child(div().text_size(px(11.)).mt(px(2.)).text_color(col(TEXT_LIGHT)).child(count.to_owned()))
        });

        div()
            .id("page-organize")
            .flex().flex_col()
            .child(
                div()
                    .id("organize-header")
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .mb(px(16.))
                    .child(page_title("🔍 智能整理", "选择工具开始清理"))
                    .child(
                        Button::new("organize-rescan")
                            .outline()
                            .icon(IconName::Redo)
                            .label("重新扫描"),
                    ),
            )
            .child(
                div()
                    .id("tool-grid")
                    .grid()
                    .grid_cols(3)
                    .gap(px(12.))
                    .mb(px(16.))
                    .children(tool_cards.collect::<Vec<_>>()),
            )
            .child(
                div()
                    .id("stories-panel")
                    .flex_1()
                    .bg(col(BG_WHITE))
                    .rounded_lg()
                    .px(px(18.))
                    .py(px(16.))
                    .shadow_sm()
                    .child(
                        div()
                            .id("stories-label")
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(TEXT_SECONDARY))
                            .mb(px(10.))
                            .child("📖 自动生成的故事"),
                    )
                    .child(
                        div()
                            .id("stories-scroll")
                            .flex().flex_row()
                            .gap(px(12.))
                            .overflow_x_scroll()
                            .children(story_cards.collect::<Vec<_>>()),
                    ),
            )
    }

    // ===== 相册库页 =====
    fn page_library(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;

        // 目录树面板（真实数据）
        let tree_panel = div()
            .id("tree-panel")
            .w(px(220.))
            .flex_none()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(10.))
            .bg(col(BG_WHITE))
            .rounded_lg()
            .shadow_sm()
            .child(render_folder_tree(state, app.clone()));

        let visible = state.data.visible_photos();
        let tiles = visible
            .iter()
            .enumerate()
            .map(|(idx, photo)| {
                let app = app.clone();
                let is_selected = state.selected.contains(&idx);
                let location = photo.location.clone().unwrap_or_default();
                let name = photo.file_name.clone();
                // 有缩略图用真实图片，否则用色块占位（M0 视觉）
                let bg_hue = stable_hue(&photo.path.to_string_lossy());
                Button::new(("photo", idx))
                    .p(px(0.))
                    .relative()
                    .overflow_hidden()
                    .h(px(150.))
                    .w_full()
                    .bg(photo_block_bg(bg_hue))
                    .border_2()
                    .when(is_selected, |b| b.border_color(col(GOLD)))
                    .when(!is_selected, |b| b.border_color(transparent_black()))
                    .child(
                        div()
                            .id(("photocontent", idx))
                            .size_full()
                            .items_center()
                            .justify_center()
                            .flex().flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .id(("photoicon", idx))
                                    .size(px(32.))
                                    .rounded_full()
                                    .bg(photo_icon_bg(bg_hue))
                                    .text_color(rgba(255, 255, 255, 0.95))
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(location.chars().next().unwrap_or('图').to_string()),
                            )
                            .child(
                                div()
                                    .id(("photoname", idx))
                                    .text_size(px(10.))
                                    .text_color(rgba(255, 255, 255, 0.9))
                                    .child(name),
                            ),
                    )
                    .child(
                        div()
                            .id(("photolabel", idx))
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .px(px(8.))
                            .py(px(2.))
                            .bg(rgba(26, 38, 52, 0.7))
                            .text_color(gpui::white())
                            .text_size(px(10.))
                            .child(location),
                    )
                    .on_click(move |ev, window, cx| {
                        if ev.modifiers().control || ev.modifiers().platform {
                            app.update(cx, |s, cx| {
                                if s.selected.contains(&idx) {
                                    s.selected.remove(&idx);
                                } else {
                                    s.selected.insert(idx);
                                }
                                cx.notify();
                            });
                        } else {
                            open_photo_dialog(&app, idx, window, cx);
                        }
                    })
            })
            .collect::<Vec<_>>();

        // 空态：未扫描时给出引导
        let grid_body: AnyElement = if visible.is_empty() {
            div()
                .id("library-empty")
                .flex_1()
                .flex().flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .child(Icon::new(IconName::FolderOpen).size(px(48.)).text_color(col(BORDER_LIGHT)))
                .child(
                    div()
                        .text_size(px(15.))
                        .text_color(col(TEXT_SECONDARY))
                        .child("相册还是空的"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(col(TEXT_LIGHT))
                        .child("点击右上角「选择目录」开始建立索引"),
                )
                .into_any_element()
        } else {
            div()
                .id("photo-grid-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .grid()
                .grid_cols(4)
                .gap(px(12.))
                .content_start()
                .children(tiles)
                .into_any_element()
        };

        let grid_panel = div()
            .id("grid-panel")
            .flex_1()
            .min_h_0()
            .flex().flex_col()
            .p(px(16.))
            .bg(col(BG_WHITE))
            .rounded_lg()
            .shadow_sm()
            .child(
                div()
                    .id("grid-toolbar")
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .mb(px(12.))
                    .text_size(px(13.))
                    .child(
                        div()
                            .id("grid-info")
                            .text_color(col(TEXT_SECONDARY))
                            .child("共 ")
                            .child(
                                div()
                                    .id("grid-count")
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(col(PRIMARY))
                                    .child(format!("{}", visible.len())),
                            )
                            .child(" 张照片"),
                    )
                    .child(
                        div()
                            .id("grid-selectinfo")
                            .text_size(px(12.))
                            .text_color(col(TEXT_SECONDARY))
                            .when(!state.selected.is_empty(), |d| {
                                d.child(format!("已选 {} 张 · Ctrl+点按多选", state.selected.len()))
                            }),
                    ),
            )
            .child(grid_body);

        // 扫描状态条
        let status_bar = if state.is_scanning || !state.scan_status.is_empty() {
            div()
                .id("scan-status")
                .mb(px(8.))
                .px(px(12.))
                .py(px(6.))
                .rounded_md()
                .bg(col(GOLD_PALE))
                .text_size(px(12.))
                .text_color(col(PRIMARY))
                .child(state.scan_status.clone())
        } else {
            div().id("scan-status-none")
        };

        // 「选择目录」按钮：点击后弹出原生目录对话框并启动后台扫描
        let app_for_scan = app.clone();
        let scanning = state.is_scanning;
        let scan_btn = Button::new("choose-dir")
            .primary()
            .icon(IconName::FolderOpen)
            .label(if scanning { "扫描中…" } else { "选择目录" })
            .on_click(move |_ev, _window, cx| {
                if scanning {
                    return;
                }
                // 原生目录选择对话框（rfd 内部会切到 UI 线程，同步等待即可）
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("选择照片目录")
                    .pick_folder()
                {
                    let path = folder.to_path_buf();
                    app_for_scan.update(cx, |s, cx| {
                        s.start_scan(path, cx);
                    });
                }
            });

        div()
            .id("page-library")
            .flex().flex_col()
            .child(status_bar)
            .child(
                div()
                    .id("library-header")
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .mb(px(16.))
                    .child(page_title("🖼️ 相册库", header_label(state)))
                    .child(
                        div().id("library-actions").flex().flex_row().gap(px(8.)).children(vec![
                            scan_btn,
                            header_btn(IconName::CaseSensitive, "重命名", TEXT_SECONDARY),
                            header_btn(IconName::FolderOpen, "归档", TEXT_SECONDARY),
                        ]),
                    ),
            )
            .child(
                div()
                    .id("library-layout")
                    .flex_1()
                    .min_h_0()
                    .flex().flex_row()
                    .gap(px(16.))
                    .child(tree_panel)
                    .child(grid_panel),
            )
    }

    fn page_placeholder(&self, icon: IconName, text: &'static str) -> impl IntoElement + 'static {
        div()
            .id("page-placeholder")
            .flex_1()
            .items_center()
            .justify_center()
            .flex().flex_col()
            .gap(px(10.))
            .text_color(col(TEXT_LIGHT))
            .child(Icon::new(icon).size(px(48.)).text_color(col(BORDER_LIGHT)))
            .child(div().text_size(px(15.)).child(text.to_owned()))
    }
}

fn header_btn(icon: IconName, label: &'static str, color: u32) -> Button {
    Button::new(label)
        .outline()
        .icon(icon)
        .label(label)
        .text_color(col(color))
}

/// 根据路径稳定生成色相，保证同一照片每次显示相同占位色。
fn stable_hue(s: &str) -> f32 {
    let mut h: u32 = 0;
    for b in s.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    (h % 360) as f32
}

/// 首页副标题。
fn subtitle_for_home(state: &AppState) -> String {
    if !state.data.loaded {
        "尚未建立索引 · 前往相册库开始".to_string()
    } else {
        format!("已索引 {} 张照片 · 全部存储在本地", state.data.total_count)
    }
}

/// 相册库标题栏右侧的说明文字。
fn header_label(state: &AppState) -> String {
    if !state.data.loaded {
        return "尚未建立索引".to_string();
    }
    let count = state.data.visible_photos().len();
    let folder = state
        .active_folder_id
        .as_deref()
        .and_then(|id| {
            crate::ui::library::find_folder(&state.data.folders, id).map(|f| f.name.clone())
        })
        .unwrap_or_else(|| "全部照片".to_string());
    format!("{} · {} 张", folder, count)
}

// ===== 目录树（递归，基于真实索引） =====
fn render_folder_tree(state: &AppState, app: Entity<AppState>) -> impl IntoElement + 'static {
    render_folder_nodes(&state.data.folders, state, app)
}

fn render_folder_nodes(
    folders: &[crate::core::model::Folder],
    state: &AppState,
    app: Entity<AppState>,
) -> AnyElement {
    let mut out: Vec<AnyElement> = vec![];
    for folder in folders {
        let key = stable_id(&folder.id);
        let is_active = state.active_folder_id.as_deref() == Some(folder.id.as_str());
        let has_children = !folder.children.is_empty();
        let expanded = state.folder_expanded.contains(&folder.id);
        let folder_id = folder.id.clone();
        let folder_path = folder.path.clone();
        let name = folder.name.clone();
        let count = folder.photo_count;

        let click_app = app.clone();
        let fid = folder_id.clone();
        let fpath = folder_path.clone();
        let row = Button::new(("folder", key))
            .ghost()
            .w_full()
            .justify_start()
            .px(px(10.))
            .py(px(6.))
            .gap(px(8.))
            .rounded_md()
            .when(is_active, |b| b.bg(col(GOLD_PALE)).text_color(col(PRIMARY)))
            .when(!is_active, |b| b.text_color(col(TEXT_SECONDARY)))
            .child(
                div()
                    .id(("folder-arrow", key))
                    .w(px(16.))
                    .flex_none()
                    .items_center()
                    .child(
                        if has_children {
                            Icon::new(IconName::ChevronRight)
                                .size_3()
                                .text_color(col(TEXT_LIGHT))
                                .when(expanded, |i| i.rotate(gpui::Radians(std::f32::consts::FRAC_PI_2)))
                        } else {
                            Icon::new(IconName::ChevronRight)
                                .size_3()
                                .text_color(transparent_black())
                        },
                    ),
            )
            .child(Icon::new(IconName::FolderClosed).size_4().text_color(col(PRIMARY_LIGHTER)))
            .child(name.clone())
            .child(
                div()
                    .id(("folder-count", key))
                    .ml_auto()
                    .text_size(px(11.))
                    .text_color(col(TEXT_LIGHT))
                    .child(count.to_string()),
            )
            .on_click(move |_ev, _w, cx| {
                click_app.update(cx, |s, cx| {
                    if s.active_folder_id.as_deref() == Some(fid.as_str()) {
                        s.active_folder_id = None;
                        s.data.active_folder = None;
                    } else {
                        s.active_folder_id = Some(fid.clone());
                        s.data.active_folder = Some(fpath.clone());
                    }
                    if has_children {
                        if !s.folder_expanded.remove(&fid) {
                            s.folder_expanded.insert(fid.clone());
                        }
                    }
                    s.selected.clear();
                    cx.notify();
                });
            });
        out.push(row.into_any_element());

        if has_children && expanded {
            out.push(
                div()
                    .id(("folder-children", key))
                    .pl(px(16.))
                    .w_full()
                    .flex().flex_col()
                    .child(render_folder_nodes(&folder.children, state, app.clone()))
                    .into_any_element(),
            );
        }
    }
    div().id("folder-list").w_full().flex().flex_col().children(out).into_any_element()
}

/// 把字符串 id 转成稳定的 usize，供 gpui ElementId 使用。
fn stable_id(s: &str) -> usize {
    let mut h: u64 = 14695981039346656037;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h as usize
}

// ===== 照片详情对话框 =====
fn open_photo_dialog(app: &Entity<AppState>, idx: usize, window: &mut Window, cx: &mut App) {
    // 取可见列表里的第 idx 张真实照片，并把字段克隆为 owned 以进入 'static 闭包
    let photo = {
        let state = app.read(cx);
        match state.data.visible_photos().get(idx) {
            Some(p) => (*p).clone(),
            None => return,
        }
    };

    let name = photo.file_name.clone();
    let date = photo.date_label();
    let size = photo.size_label();
    let file_size = photo.file_size_label();
    let device = photo.device.clone().unwrap_or_else(|| "未知设备".to_string());
    let location = photo.location.clone().unwrap_or_else(|| "未知地点".to_string());
    let tags: Vec<String> = photo.tag_list().into_iter().map(|s| s.to_string()).collect();
    let hue = stable_hue(&photo.path.to_string_lossy());
    let has_thumb = photo.thumbnail_path.as_ref().map(|p| p.exists()).unwrap_or(false);

    let preview_hint = if has_thumb { "已生成缩略图缓存" } else { "暂无缩略图" };
    window.open_dialog(cx, move |dialog: Dialog, _w, _cx| {
        let build_tags = || {
            tags.iter()
                .enumerate()
                .map(|(ti, t)| {
                    div()
                        .id(("tag", ti))
                        .bg(col(GOLD_PALE))
                        .text_color(col(PRIMARY))
                        .px(px(10.))
                        .py(px(2.))
                        .rounded_full()
                        .text_size(px(11.))
                        .child(t.clone())
                        .into_any_element()
                })
                .collect::<Vec<AnyElement>>()
        };
        dialog
            .title(format!("照片详情 · {}", name))
            .w(px(880.))
            .h(px(540.))
            .py(px(16.))
            .child(
                div()
                    .id("photo-dialog-body")
                    .flex_1()
                    .min_h_0()
                    .flex().flex_row()
                    .child(
                        div()
                            .id("dialog-image")
                            .flex_1()
                            .min_h_0()
                            .items_center()
                            .justify_center()
                            .bg(col(BG_WARM))
                            .rounded_md()
                            .child(
                                div()
                                    .id("dialog-preview")
                                    .size(px(340.))
                                    .rounded_lg()
                                    .bg(photo_block_bg(hue))
                                    .items_center()
                                    .justify_center()
                                    .flex().flex_col()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .id("dialog-loc")
                                            .text_size(px(18.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgba(255, 255, 255, 0.95))
                                            .child(location.clone()),
                                    )
                                    .child(
                                        div()
                                            .id("dialog-sim")
                                            .text_size(px(12.))
                                            .text_color(rgba(255, 255, 255, 0.75))
                                            .child(preview_hint),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("dialog-info")
                            .w(px(300.))
                            .min_w(px(300.))
                            .px(px(20.))
                            .py(px(12.))
                            .border_l_1()
                            .border_color(col(BORDER_LIGHT))
                            .overflow_y_scroll()
                            .flex().flex_col()
                            .child(info_row("文件名", name.clone()))
                            .child(info_row("拍摄日期", date.clone()))
                            .child(info_row("尺寸", size.clone()))
                            .child(info_row("文件大小", file_size.clone()))
                            .child(info_row("设备", device.clone()))
                            .child(info_row("地点", location.clone()))
                            .child(info_tag_section("🏷️ 标签", build_tags()))
                            .child(
                                div()
                                    .id("dialog-actions")
                                    .mt_4()
                                    .flex().flex_row()
                                    .gap(px(6.))
                                    .children(vec![
                                        dialog_btn(IconName::Star, "收藏", PRIMARY),
                                        dialog_btn(IconName::Info, "标签", TEXT_SECONDARY),
                                        dialog_btn(IconName::FolderOpen, "移动", TEXT_SECONDARY),
                                        dialog_btn(IconName::Delete, "", DANGER),
                                    ]),
                            ),
                    ),
            )
            .on_ok(|_ev, window, cx| {
                window.close_dialog(cx);
                true
            })
    });
}

fn info_row(label: &'static str, value: impl Into<SharedString>) -> AnyElement {
    div()
        .flex().flex_row()
        .justify_between()
        .py(px(6.))
        .child(div().text_color(col(TEXT_SECONDARY)).child(label))
        .child(div().text_color(col(PRIMARY)).font_weight(FontWeight::MEDIUM).child(value.into()))
        .into_any_element()
}

fn info_tag_section(title: &'static str, tags: Vec<AnyElement>) -> AnyElement {
    div()
        .id("tag-section")
        .mt_3()
        .child(
            div()
                .id("tag-label")
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(col(TEXT_SECONDARY))
                .mb(px(6.))
                .child(title.to_owned()),
        )
        .child(
            div()
                .id("tag-list")
                .flex().flex_row()
                .flex_wrap()
                .gap(px(6.))
                .children(tags),
        )
        .into_any_element()
}

fn dialog_btn(icon: IconName, label: &'static str, color: u32) -> Button {
    let mut b = Button::new(label)
        .outline()
        .icon(icon)
        .text_color(col(color));
    if !label.is_empty() {
        b = b.label(label);
    }
    b
}
