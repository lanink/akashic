use std::borrow::Cow;
use std::collections::HashSet;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::dialog::Dialog;
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::Root;
use gpui_component::WindowExt;

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

// ===== 照片数据（设计稿中的模拟数据） =====
struct PhotoDef {
    name: &'static str,
    date: &'static str,
    size: &'static str,
    file_size: &'static str,
    device: &'static str,
    location: &'static str,
    tags: &'static [&'static str],
    hue: f32, // 缩略图色相（0..360）
}

const PHOTOS: &[PhotoDef] = &[
    PhotoDef { name: "IMG_20240815_094523.jpg", date: "2024-08-15 09:45", size: "4032×3024", file_size: "4.2 MB", device: "iPhone 15 Pro", location: "日本 · 富士山", tags: &["旅行", "风景", "富士山", "2024"], hue: 8.0 },
    PhotoDef { name: "IMG_20240815_102134.jpg", date: "2024-08-15 10:21", size: "4032×3024", file_size: "3.8 MB", device: "iPhone 15 Pro", location: "日本 · 河口湖", tags: &["旅行", "风景", "河口湖"], hue: 200.0 },
    PhotoDef { name: "IMG_20240815_143022.jpg", date: "2024-08-15 14:30", size: "3024×4032", file_size: "5.1 MB", device: "iPhone 15 Pro", location: "日本 · 浅草寺", tags: &["旅行", "寺庙", "浅草"], hue: 338.0 },
    PhotoDef { name: "IMG_20240816_082145.jpg", date: "2024-08-16 08:21", size: "4032×3024", file_size: "6.3 MB", device: "iPhone 15 Pro", location: "日本 · 新干线", tags: &["旅行", "列车", "新干线"], hue: 255.0 },
    PhotoDef { name: "IMG_20240816_120033.jpg", date: "2024-08-16 12:00", size: "4032×3024", file_size: "4.7 MB", device: "iPhone 15 Pro", location: "日本 · 京都", tags: &["旅行", "美食", "京都"], hue: 40.0 },
    PhotoDef { name: "IMG_20240816_163845.jpg", date: "2024-08-16 16:38", size: "3024×4032", file_size: "3.2 MB", device: "iPhone 15 Pro", location: "日本 · 伏见稻荷", tags: &["旅行", "神社", "鸟居"], hue: 350.0 },
    PhotoDef { name: "IMG_20240817_091234.jpg", date: "2024-08-17 09:12", size: "4032×3024", file_size: "5.8 MB", device: "iPhone 15 Pro", location: "日本 · 奈良", tags: &["旅行", "鹿", "奈良"], hue: 150.0 },
    PhotoDef { name: "IMG_20240817_141522.jpg", date: "2024-08-17 14:15", size: "4032×3024", file_size: "4.4 MB", device: "iPhone 15 Pro", location: "日本 · 大阪", tags: &["旅行", "城市", "大阪"], hue: 280.0 },
    PhotoDef { name: "IMG_20240818_103045.jpg", date: "2024-08-18 10:30", size: "3024×4032", file_size: "3.9 MB", device: "iPhone 15 Pro", location: "日本 · 东京塔", tags: &["旅行", "地标", "东京"], hue: 60.0 },
    PhotoDef { name: "IMG_20240818_193022.jpg", date: "2024-08-18 19:30", size: "4032×3024", file_size: "2.8 MB", device: "iPhone 15 Pro", location: "日本 · 涩谷", tags: &["旅行", "夜景", "涩谷"], hue: 230.0 },
    PhotoDef { name: "IMG_20240819_075634.jpg", date: "2024-08-19 07:56", size: "4032×3024", file_size: "6.1 MB", device: "iPhone 15 Pro", location: "日本 · 镰仓", tags: &["旅行", "海岸", "镰仓"], hue: 190.0 },
    PhotoDef { name: "IMG_20240819_123045.jpg", date: "2024-08-19 12:30", size: "4032×3024", file_size: "3.5 MB", device: "iPhone 15 Pro", location: "日本 · 江之岛", tags: &["旅行", "海岛", "江之岛"], hue: 122.0 },
];

// ===== 照片缩略图配色 =====
fn photo_block_bg(hue: f32) -> Hsla {
    gpui::hsla(hue / 360.0, 0.36, 0.74, 1.0)
}
fn photo_icon_bg(hue: f32) -> Hsla {
    gpui::hsla(hue / 360.0, 0.46, 0.42, 1.0)
}

// ===== 相册目录 =====
struct FolderNode {
    name: &'static str,
    count: usize,
    children: Vec<FolderNode>,
}

fn folder_defs() -> Vec<FolderNode> {
    vec![
        FolderNode { name: "日本旅行", count: 85, children: vec![] },
        FolderNode { name: "生日派对", count: 34, children: vec![] },
        FolderNode { name: "四姑娘山", count: 47, children: vec![] },
        FolderNode {
            name: "2024年",
            count: 328,
            children: vec![
                FolderNode { name: "青岛旅行", count: 156, children: vec![] },
                FolderNode { name: "公司团建", count: 47, children: vec![] },
            ],
        },
        FolderNode {
            name: "2023年",
            count: 892,
            children: vec![
                FolderNode { name: "三亚旅行", count: 203, children: vec![] },
                FolderNode { name: "圣诞聚餐", count: 28, children: vec![] },
            ],
        },
    ]
}

// ===== 应用全局状态 =====
struct AppState {
    page: Page,
    active_folder: usize,
    folder_expanded: Vec<bool>, // 与可展开目录一一对应
    photos: &'static [PhotoDef],
    photo_header: &'static str,
    selected: HashSet<usize>,
}

impl AppState {
    fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|_| Self {
            page: Page::Library,
            active_folder: 0,
            folder_expanded: vec![true, false], // 2024年展开、2023年收起
            photos: PHOTOS,
            photo_header: "日本旅行 · 85 张",
            selected: HashSet::new(),
        })
    }
}

// ===== 根组件 =====
struct AppRoot {
    app: Entity<AppState>,
}

impl AppRoot {
    fn new(app: Entity<AppState>) -> Self {
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

fn dot(color: u32) -> impl IntoElement + 'static {
    div().id(("dot", color)).size(px(13.)).rounded_full().bg(col(color))
}

// ===== 工具/卡片辅助 =====
fn page_title(main: &'static str, sub: &'static str) -> impl IntoElement {
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
                .child(main),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(col(TEXT_SECONDARY))
                .child(sub),
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
            Page::Home => self.page_home().into_any_element(),
            Page::Organize => self.page_organize().into_any_element(),
            Page::Library => self.page_library(state).into_any_element(),
            Page::Rename => self
                .page_placeholder(IconName::CaseSensitive, "重命名功能 — 从相册库选择照片后使用")
                .into_any_element(),
            Page::Trash => self
                .page_placeholder(IconName::Delete, "回收站共 4 张照片")
                .into_any_element(),
        }
    }
}

impl AppRoot {
    // ===== 首页 =====
    fn page_home(&self) -> impl IntoElement + 'static {
        let app = &self.app;
        let stats = vec![
            ("8,432", "总照片", PRIMARY),
            ("42.6 GB", "占用空间", PRIMARY),
            ("~400", "可清理预估", GOLD),
            ("1,247", "已归档", PRIMARY),
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
                        .child(v.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .mt(px(2.))
                        .text_color(col(TEXT_SECONDARY))
                        .child(label.to_owned()),
                )
        });

        let records = vec![
            ("批量删除", "156 张 · 2.3 GB"),
            ("重命名", "328 张 · 日本旅行"),
            ("归档", "203 张 → 三亚旅行"),
            ("清理截图", "89 张 · 0.6 GB"),
        ];
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
                .child(div().text_color(col(TEXT_SECONDARY)).child(a.to_owned()))
                .child(div().font_weight(FontWeight::SEMIBOLD).child(b.to_owned()))
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
                    .child(page_title("📊 相册概览", "上次整理：昨天 · 清理了 156 张"))
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
            .child(render_folder_tree(&folder_defs(), state, 0, app.clone()));

        let tiles = state
            .photos
            .iter()
            .enumerate()
            .map(|(idx, photo)| {
                let app = app.clone();
                let is_selected = state.selected.contains(&idx);
                Button::new(("photo", idx))
                    .p(px(0.))
                    .relative()
                    .overflow_hidden()
                    .h(px(150.))
                    .w_full()
                    .bg(photo_block_bg(photo.hue))
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
                                    .bg(photo_icon_bg(photo.hue))
                                    .text_color(rgba(255, 255, 255, 0.95))
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(photo.location.chars().next().unwrap_or('图').to_string()),
                            )
                            .child(
                                div()
                                    .id(("photoname", idx))
                                    .text_size(px(10.))
                                    .text_color(rgba(255, 255, 255, 0.9))
                                    .child(photo.name.to_owned()),
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
                            .child(photo.location.to_owned()),
                    )
                    .on_click(move |ev, window, cx| {
                        if ev.modifiers().control || ev.modifiers().platform {
                            let s = app.update(cx, |s, cx| {
                                if s.selected.contains(&idx) {
                                    s.selected.remove(&idx);
                                } else {
                                    s.selected.insert(idx);
                                }
                                cx.notify();
                            });
                            let _ = s;
                        } else {
                            open_photo_dialog(&app, idx, window, cx);
                        }
                    })
            })
            .collect::<Vec<_>>();

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
                            .child(format!("{}", state.photos.len())),
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
            .child(
                div()
                    .id("photo-grid-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .grid()
                    .grid_cols(4)
                    .gap(px(12.))
                    .content_start()
                    .children(tiles),
            );

        div()
            .id("page-library")
            .flex().flex_col()
            .child(
                div()
                    .id("library-header")
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .mb(px(16.))
                    .child(page_title("🖼️ 相册库", state.photo_header))
                    .child(
                        div().id("library-actions").flex().flex_row().gap(px(8.)).children(vec![
                            header_btn(IconName::CaseSensitive, "重命名", TEXT_SECONDARY),
                            header_btn(IconName::FolderOpen, "归档", TEXT_SECONDARY),
                            header_btn(IconName::Check, "选择", PRIMARY),
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

// ===== 目录树（递归） =====
fn render_folder_tree(
    folders: &[FolderNode],
    state: &AppState,
    base: usize,
    app: Entity<AppState>,
) -> impl IntoElement + 'static {
    let mut out: Vec<AnyElement> = vec![];
    for (i, folder) in folders.iter().enumerate() {
        let flat_idx = base + i;
        let is_active = state.active_folder == flat_idx;
        let has_children = !folder.children.is_empty();
        let expanded = if has_children {
            expandable_id(&folder_defs(), flat_idx)
                .and_then(|e| state.folder_expanded.get(e).copied())
                .unwrap_or(true)
        } else {
            false
        };

        let app = app.clone();
        let location = folder.name;
        let click_app = app.clone();
        let row = Button::new(("folder", flat_idx))
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
                    .id(("folder-arrow", flat_idx))
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
            .child(location.to_owned())
            .child(
                div()
                    .id(("folder-count", flat_idx))
                    .ml_auto()
                    .text_size(px(11.))
                    .text_color(col(TEXT_LIGHT))
                    .child(folder.count.to_string()),
            )
            .on_click(move |_ev, _w, cx| {
                let s = click_app.update(cx, |s, cx| {
                    s.active_folder = flat_idx;
                    if has_children {
                        if let Some(e) = expandable_id(&folder_defs(), flat_idx) {
                            if let Some(cur) = s.folder_expanded.get_mut(e) {
                                *cur = !*cur;
                            }
                        }
                    }
                    cx.notify();
                });
                let _ = s;
            });
        out.push(row.into_any_element());

        if has_children && expanded {
            out.push(
                div()
                    .id(("folder-children", flat_idx))
                    .pl(px(16.))
                    .w_full()
                    .flex().flex_col()
                    .child(render_folder_tree(&folder.children, state, flat_idx + 1, app))
                    .into_any_element(),
            );
        }
    }
    div().id("folder-list").w_full().flex().flex_col().children(out).into_any_element()
}

/// 计算一个扁平索引（深度优先序号）是否为可展开节点；是则返回它在 folder_expanded 中的下标
fn expandable_id(folders: &[FolderNode], target: usize) -> Option<usize> {
    let mut order = vec![];
    fn walk(folders: &[FolderNode], idx: &mut usize, order: &mut Vec<usize>) {
        for f in folders {
            if !f.children.is_empty() {
                order.push(*idx);
            }
            *idx += 1;
            walk(&f.children, idx, order);
        }
    }
    walk(folders, &mut 0, &mut order);
    order.iter().position(|&n| n == target)
}

// ===== 照片详情对话框 =====
fn open_photo_dialog(app: &Entity<AppState>, idx: usize, window: &mut Window, cx: &mut App) {
    let photo = &PHOTOS[idx];
    let build_tags = || {
        photo
            .tags
            .iter()
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
                    .child(t.to_owned())
                    .into_any_element()
            })
            .collect::<Vec<AnyElement>>()
    };

        let info_row = |label: &'static str, value: &'static str| {
        div()
            .flex().flex_row()
            .justify_between()
            .py(px(6.))
            .child(div().text_color(col(TEXT_SECONDARY)).child(label.to_owned()))
            .child(div().text_color(col(PRIMARY)).font_weight(FontWeight::MEDIUM).child(value.to_owned()))
            .into_any_element()
    };

    let _ = app;
    window.open_dialog(cx, move |dialog: Dialog, _w, _cx| {
        dialog
            .title(format!("照片详情 · {}", photo.name))
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
                                    .bg(photo_block_bg(photo.hue))
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
                                            .child(photo.location.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .id("dialog-sim")
                                            .text_size(px(12.))
                                            .text_color(rgba(255, 255, 255, 0.75))
                                            .child("模拟预览 · 实际为照片缩略图"),
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
                            .child(info_row("文件名", photo.name))
                            .child(info_row("拍摄日期", photo.date))
                            .child(info_row("尺寸", photo.size))
                            .child(info_row("文件大小", photo.file_size))
                            .child(info_row("设备", photo.device))
                            .child(info_row("地点", photo.location))
                            .child(info_tag_section("🏷️ 标签", build_tags()))
                            .child(
                                div()
                                    .id("dialog-album")
                                    .mt_3()
                                    .child(
                                        div()
                                            .id("album-label")
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(col(TEXT_SECONDARY))
                                            .mb(px(6.))
                                            .child("📂 所属相册"),
                                    )
                                    .child(div().text_size(px(13.)).text_color(col(TEXT_SECONDARY)).child("日本旅行")),
                            )
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

// ===== 启动 =====
fn main() {
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
                let state = AppState::new(cx);
                let view = cx.new(|_| AppRoot::new(state));
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

// ===== 图标资源：编译期嵌入 assets/icons 目录的 Lucide SVG =====
#[derive(Clone)]
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let file: Option<&[u8]> = match path {
            "icons/layout-dashboard.svg" => Some(include_bytes!("../assets/icons/layout-dashboard.svg")),
            "icons/settings-2.svg" => Some(include_bytes!("../assets/icons/settings-2.svg")),
            "icons/gallery-vertical-end.svg" => Some(include_bytes!("../assets/icons/gallery-vertical-end.svg")),
            "icons/case-sensitive.svg" => Some(include_bytes!("../assets/icons/case-sensitive.svg")),
            "icons/delete.svg" => Some(include_bytes!("../assets/icons/delete.svg")),
            "icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "icons/copy.svg" => Some(include_bytes!("../assets/icons/copy.svg")),
            "icons/eye-off.svg" => Some(include_bytes!("../assets/icons/eye-off.svg")),
            "icons/frame.svg" => Some(include_bytes!("../assets/icons/frame.svg")),
            "icons/file.svg" => Some(include_bytes!("../assets/icons/file.svg")),
            "icons/folder-open.svg" => Some(include_bytes!("../assets/icons/folder-open.svg")),
            "icons/book-open.svg" => Some(include_bytes!("../assets/icons/book-open.svg")),
            "icons/star.svg" => Some(include_bytes!("../assets/icons/star.svg")),
            "icons/search.svg" => Some(include_bytes!("../assets/icons/search.svg")),
            "icons/close.svg" => Some(include_bytes!("../assets/icons/close.svg")),
            "icons/chevron-right.svg" => Some(include_bytes!("../assets/icons/chevron-right.svg")),
            "icons/check.svg" => Some(include_bytes!("../assets/icons/check.svg")),
            "icons/info.svg" => Some(include_bytes!("../assets/icons/info.svg")),
            "icons/circle-user.svg" => Some(include_bytes!("../assets/icons/circle-user.svg")),
            "icons/circle-check.svg" => Some(include_bytes!("../assets/icons/circle-check.svg")),
            "icons/redo.svg" => Some(include_bytes!("../assets/icons/redo.svg")),
            "icons/plus.svg" => Some(include_bytes!("../assets/icons/plus.svg")),
            _ => None,
        };
        Ok(file.map(|bytes| Cow::Owned(bytes.to_vec())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}