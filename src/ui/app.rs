use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::*;
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::Disableable;
use gpui_component::Icon;
use gpui_component::IconName;
use gpui_component::WindowExt;

use crate::core::analyze::{AnalyzeProgress, AnalyzeReport};
use crate::core::config::Paths;
use crate::core::ops::archive::{self as ops_archive, ArchiveMode, ArchivePlan, ArchiveRule, ArchiveTreeNode};
use crate::core::ops::{rename as ops_rename, trash as ops_trash};
use crate::core::model::{Photo, ScanProgress, ScanStats};
use crate::ui::analyze::AnalyzeData;
use crate::ui::editor::open_photo_editor;
use crate::ui::library::LibraryData;
use crate::ui::ops::OpsData;

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

// ===== 相册库网格 =====
/// 单张缩略图宽度（px）：与整理页 result tile 同款固定宽度，避免依赖父容器测量。
const GRID_TILE_WIDTH: f32 = 180.;
/// 单张缩略图高度（px）。
const GRID_TILE_HEIGHT: f32 = 150.;

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
    Archive,
    Trash,
    Settings,
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
    /// 多选集合存照片 id，而非「当前可见列表下标」。
    /// 列表按拍摄时间倒序，任何扫描/恢复/撤销都会重排，存下标会删错照片。
    selected: HashSet<i64>,
    /// 扫描状态文字
    scan_status: String,
    is_scanning: bool,
    /// 后台扫描进度接收端（Send，可放 Entity）
    progress_rx: Option<Receiver<ScanProgress>>,
    /// M2 分析结果快照
    analyze_data: AnalyzeData,
    /// 后台分析进度接收端
    analyze_rx: Option<Receiver<AnalyzeProgress>>,
    is_analyzing: bool,
    analyze_status: String,
    /// 扫描完成但分析正忙时排队，避免这次自动分析被静默丢弃。
    pending_analysis: bool,
    /// 整理页当前打开的工具列表（None 表示工具卡片总览）
    active_tool: Option<OrganizeTool>,
    /// M3 回收站/操作日志快照
    ops_data: OpsData,
    /// 状态提示（批量操作结果）
    ops_status: String,
    /// 重命名模板与前缀（重命名页输入）
    rename_pattern: String,
    rename_prefix: String,
    /// 最近一次可撤销的重命名/删除操作 id
    last_rename_op: Option<i64>,
    last_trash_op: Option<i64>,
    /// M4 智能归档配置与预览
    archive_dest: PathBuf,
    archive_rule: ArchiveRule,
    archive_mode: ArchiveMode,
    /// true=全部可见照片，false=仅相册库选中照片
    archive_scope_all: bool,
    archive_plan: Option<ArchivePlan>,
    last_archive_op: Option<i64>,
    /// M5 启动/刷新索引失败信息（错误态横幅）
    load_error: Option<String>,
    /// M5 最近一次后台任务失败信息（错误态横幅）
    last_error: Option<String>,
    /// M5 回收站保留天数，设置页可改并持久化到 settings
    retention_days: i64,
}

/// 整理页的 4 个分析工具。
#[derive(Clone, Copy, PartialEq)]
pub enum OrganizeTool {
    Similar,
    Blurry,
    Screenshot,
    Large,
}

impl AppState {
    fn new(cx: &mut App, paths: Paths) -> Entity<Self> {
        cx.new(|_| {
            // 启动时尝试从已有索引加载；失败则用空快照（不影响 UI 启动）
            let db_path = paths.db_path.clone();
            let mut data = LibraryData::default();
            let mut analyze_data = AnalyzeData::default();
            let mut ops_data = OpsData::default();
            let mut archive_dest = PathBuf::new();
            let mut archive_rule = ArchiveRule::default();
            let mut archive_mode = ArchiveMode::default();
            let mut retention_days = ops_trash::DEFAULT_RETENTION_DAYS;
            let mut load_error: Option<String> = None;
            match crate::core::db::open(&db_path) {
                Ok(conn) => {
                    data = LibraryData::reload(&conn).unwrap_or_default();
                    analyze_data = AnalyzeData::reload(&conn).unwrap_or_default();
                    ops_data = OpsData::reload(&conn).unwrap_or_default();
                    if let Some(v) = crate::core::db::get_setting(&conn, "archive.dest").ok().flatten() {
                        archive_dest = PathBuf::from(v);
                    }
                    if let Some(v) = crate::core::db::get_setting(&conn, "archive.rule").ok().flatten() {
                        archive_rule = ArchiveRule::from_str(&v);
                    }
                    if let Some(v) = crate::core::db::get_setting(&conn, "archive.mode").ok().flatten() {
                        archive_mode = ArchiveMode::from_str(&v);
                    }
                    retention_days = ops_trash::retention_days(&conn);
                }
                Err(e) => load_error = Some(format!("索引库打开失败：{e}")),
            }
            let mut s = Self {
                page: Page::Library,
                paths,
                data,
                active_folder_id: None,
                folder_expanded: HashSet::new(),
                selected: HashSet::new(),
                scan_status: String::new(),
                is_scanning: false,
                progress_rx: None,
                analyze_data,
                analyze_rx: None,
                is_analyzing: false,
                analyze_status: String::new(),
                pending_analysis: false,
                active_tool: None,
                ops_data,
                ops_status: String::new(),
                rename_pattern: "{前缀}_{日期}_{序号:3}".to_string(),
                rename_prefix: String::new(),
                last_rename_op: None,
                last_trash_op: None,
                archive_dest,
                archive_rule,
                archive_mode,
                archive_scope_all: true,
                archive_plan: None,
                last_archive_op: None,
                load_error,
                last_error: None,
                retention_days,
            };
            s.refresh_archive_plan();
            s
        })
    }

    /// 统一刷新三个数据快照（批量操作后调用）；失败时记录错误态且不吞掉旧快照。
    fn reload_all(&mut self) {
        // 保留当前目录过滤与关键词：LibraryData::reload 会重置它们，
        // 不回填会造成「目录树仍高亮旧目录、网格却显示全部照片」。
        let active_folder = self.data.active_folder.clone();
        let keyword = self.data.keyword.clone();
        let active_folder_id = self.active_folder_id.clone();
        match crate::core::db::open(&self.paths.db_path) {
            Ok(conn) => {
                match LibraryData::reload(&conn) {
                    Ok(mut d) => {
                        d.active_folder = active_folder;
                        d.keyword = keyword;
                        self.data = d;
                    }
                    Err(e) => self.last_error = Some(format!("刷新相册库失败：{e}")),
                }
                match AnalyzeData::reload(&conn) {
                    Ok(d) => self.analyze_data = d,
                    Err(e) => self.last_error = Some(format!("刷新分析结果失败：{e}")),
                }
                match OpsData::reload(&conn) {
                    Ok(d) => self.ops_data = d,
                    Err(e) => self.last_error = Some(format!("刷新操作记录失败：{e}")),
                }
                // 选中的目录若已不存在（例如被归档/清空），清掉高亮避免与网格不符。
                if let Some(id) = &active_folder_id {
                    if crate::ui::library::find_folder(&self.data.folders, id).is_none() {
                        self.active_folder_id = None;
                        self.data.active_folder = None;
                    }
                }
                self.load_error = None;
            }
            Err(e) => self.load_error = Some(format!("刷新索引失败：{e}")),
        }
        self.selected.clear();
    }

    /// 当前多选对应的照片 id（与列表顺序无关）。
    fn selected_photo_ids(&self) -> Vec<i64> {
        self.selected.iter().copied().collect()
    }

    /// 批量删除选中照片到回收站（同步执行，数量通常不大）。
    fn batch_trash(&mut self) {
        let ids = self.selected_photo_ids();
        if ids.is_empty() {
            self.ops_status = "请先选择照片（Ctrl+点按多选）".to_string();
            return;
        }
        let count = ids.len();
        let paths = self.paths.clone();
        let result = crate::core::db::open(&paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_trash::trash_photos(&paths, &conn, &ids));
        match result {
            Ok(r) => {
                self.last_trash_op = Some(r.op_id);
                self.ops_status = format!(
                    "已将 {} 张移动到回收站（{:.1} MB）· 可在回收站恢复或撤销",
                    count,
                    r.bytes as f64 / 1024.0 / 1024.0
                );
                self.selected.clear();
                self.reload_all();
            }
            Err(e) => self.ops_status = format!("删除失败：{e}"),
        }
    }

    /// 批量重命名选中照片（用当前 pattern/prefix）。
    fn batch_rename(&mut self) {
        let ids = self.selected_photo_ids();
        if ids.is_empty() {
            self.ops_status = "请先选择照片（Ctrl+点按多选）".to_string();
            return;
        }
        let pattern = self.rename_pattern.clone();
        let prefix = self.rename_prefix.clone();
        let paths = self.paths.clone();
        let result = crate::core::db::open(&paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_rename::execute(&conn, &ids, &pattern, &prefix));
        match result {
            Ok(r) => {
                self.last_rename_op = Some(r.op_id);
                self.ops_status = format!("已重命名 {} 张 · 可点「撤销上次重命名」恢复", r.count);
                self.selected.clear();
                self.reload_all();
            }
            Err(e) => self.ops_status = format!("重命名失败：{e}"),
        }
    }

    /// 撤销最近一次重命名。
    fn undo_rename(&mut self) {
        let Some(op_id) = self.last_rename_op else {
            self.ops_status = "没有可撤销的重命名操作".to_string();
            return;
        };
        let result = crate::core::db::open(&self.paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_rename::undo(&conn, op_id));
        match result {
            Ok(n) => {
                self.ops_status = format!("已撤销重命名，恢复 {n} 张");
                self.last_rename_op = None;
                self.reload_all();
            }
            Err(e) => self.ops_status = format!("撤销失败：{e}"),
        }
    }

    /// 撤销最近一次删除。
    fn undo_trash(&mut self) {
        let Some(op_id) = self.last_trash_op else {
            self.ops_status = "没有可撤销的删除操作".to_string();
            return;
        };
        let paths = self.paths.clone();
        let result = crate::core::db::open(&paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_trash::undo_trash(&paths, &conn, op_id));
        match result {
            Ok(n) if n > 0 => {
                self.ops_status = format!("已撤销删除，恢复 {n} 张");
                self.last_trash_op = None;
                self.reload_all();
            }
            Ok(_) => {
                // 清空/清理后仍点撤销时，不能报「恢复 0 张」当作成功。
                self.ops_status = "没有可恢复的项目（可能已被清空或过期清理）".to_string();
                self.last_trash_op = None;
                self.reload_all();
            }
            Err(e) => self.ops_status = format!("撤销失败：{e}"),
        }
    }

    /// 当前归档范围对应的照片 id。
    fn archive_ids(&self) -> Vec<i64> {
        if self.archive_scope_all {
            self.data.visible_photos().iter().map(|p| p.id).collect()
        } else {
            self.selected_photo_ids()
        }
    }

    /// 持久化归档配置到 settings 表（个人本地，重启后保留）。
    fn persist_archive_settings(&self) {
        if let Ok(conn) = crate::core::db::open(&self.paths.db_path) {
            let _ = crate::core::db::set_setting(&conn, "archive.rule", self.archive_rule.as_str());
            let _ = crate::core::db::set_setting(&conn, "archive.mode", self.archive_mode.as_str());
            if !self.archive_dest.as_os_str().is_empty() {
                let _ = crate::core::db::set_setting(
                    &conn,
                    "archive.dest",
                    &self.archive_dest.to_string_lossy(),
                );
            }
        }
    }

    /// 重新生成归档目录树预览（只读，不落盘）。
    /// 未显式选择目标目录时，默认落到「扫描根目录的同级 / 归档」，避免归档目录被后续扫描重复索引。
    fn refresh_archive_plan(&mut self) {
        let ids = self.archive_ids();
        let rule = self.archive_rule;
        let mode = self.archive_mode;
        let db_path = self.paths.db_path.clone();
        let data_dir = self.paths.data_dir.clone();
        let chosen = self.archive_dest.clone();
        let result = crate::core::db::open(&db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| {
                let dest = if !chosen.as_os_str().is_empty() {
                    chosen.clone()
                } else {
                    crate::core::db::list_scan_roots(&conn)
                        .ok()
                        .and_then(|roots| {
                            roots.first().map(|r| {
                                let base = r
                                    .path
                                    .parent()
                                    .map(|p| p.to_path_buf())
                                    .unwrap_or_else(|| r.path.clone());
                                base.join("归档")
                            })
                        })
                        .unwrap_or_else(|| data_dir.join("归档"))
                };
                ops_archive::plan(&conn, &ids, rule, mode, &dest)
            });
        match result {
            Ok(p) => self.archive_plan = Some(p),
            Err(e) => {
                self.archive_plan = None;
                self.ops_status = format!("生成归档预览失败：{e}");
            }
        }
    }

    /// 设置归档目标目录（原生对话框在调用方、且在 `update` 之外打开）。
    /// 目标落在扫描根目录内部会导致归档产物被下次扫描重复索引，直接拒绝。
    fn set_archive_dest(&mut self, dest: PathBuf) {
        let roots = crate::core::db::open(&self.paths.db_path)
            .ok()
            .and_then(|c| crate::core::db::list_scan_roots(&c).ok())
            .unwrap_or_default();
        if let Some(r) = roots.iter().find(|r| dest.starts_with(&r.path)) {
            let msg = format!("归档目标不能位于扫描目录内：{}", r.path.display());
            self.ops_status = msg.clone();
            self.last_error = Some(msg);
            return;
        }
        self.archive_dest = dest;
        self.persist_archive_settings();
        self.refresh_archive_plan();
        self.ops_status = format!("归档目标：{}", self.archive_dest.display());
        self.last_error = None;
    }

    /// 执行归档：用刚刷新的计划落盘（保证目录树预览与实际一致）。
    fn execute_archive(&mut self) {
        self.refresh_archive_plan();
        let Some(plan) = self.archive_plan.clone() else {
            self.ops_status = "无法生成归档计划，请先建立索引".to_string();
            return;
        };
        if plan.items.is_empty() {
            self.ops_status = "当前范围没有可归档的照片".to_string();
            return;
        }
        let result = crate::core::db::open(&self.paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_archive::execute(&conn, &plan));
        match result {
            Ok(r) => {
                self.last_archive_op = Some(r.op_id);
                self.ops_status = format!(
                    "已{}归档 {} 张到 {} · 可点「撤销上次归档」恢复",
                    if plan.mode == ArchiveMode::Move { "移动" } else { "复制" },
                    r.count,
                    r.dest_root.display()
                );
                self.selected.clear();
                self.reload_all();
                self.refresh_archive_plan();
            }
            Err(e) => self.ops_status = format!("归档失败：{e}"),
        }
    }

    /// 撤销最近一次归档。
    fn undo_archive(&mut self) {
        let Some(op_id) = self.last_archive_op else {
            self.ops_status = "没有可撤销的归档操作".to_string();
            return;
        };
        let result = crate::core::db::open(&self.paths.db_path)
            .map_err(anyhow::Error::from)
            .and_then(|conn| ops_archive::undo(&conn, op_id));
        match result {
            Ok(n) => {
                self.ops_status = format!("已撤销归档，处理 {n} 张");
                self.last_archive_op = None;
                self.reload_all();
                self.refresh_archive_plan();
            }
            Err(e) => self.ops_status = format!("撤销归档失败：{e}"),
        }
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

        // UI 侧：定时轮询进度通道，收到 Finished/Failed 后刷新数据快照。
        // 关键：每轮必须排空 channel。后台每张照片一条进度时，若每 150ms 只收一条，
        // 5000 张会把 Finished 挤到 ~12 分钟后，界面永远停在「扫描中…」。
        cx.spawn(async move |entity, app: &mut AsyncApp| {
            loop {
                gpui::background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;

                let messages = match entity
                    .update(app, |this: &mut Self, _cx| {
                        let Some(rx) = &this.progress_rx else {
                            return Vec::new();
                        };
                        let mut out = Vec::new();
                        let mut disconnected = false;
                        loop {
                            match rx.try_recv() {
                                Ok(p) => out.push(p),
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    disconnected = true;
                                    break;
                                }
                            }
                        }
                        let has_terminal = out.iter().any(|m| {
                            matches!(
                                m,
                                ScanProgress::Finished { .. } | ScanProgress::Failed { .. }
                            )
                        });
                        if disconnected && !has_terminal {
                            out.push(ScanProgress::Failed {
                                message: "扫描线程意外退出".to_string(),
                            });
                        }
                        out
                    })
                    .ok()
                {
                    Some(v) => v,
                    None => break, // entity 已释放，不要空转
                };
                if messages.is_empty() {
                    continue;
                }

                let done = entity
                    .update(app, |this: &mut Self, cx| {
                        let mut done = false;
                        for p in messages.iter().cloned() {
                            match p {
                                ScanProgress::Started { root } => {
                                    this.scan_status = format!("正在扫描 {} …", root.display());
                                }
                                ScanProgress::Scanning { processed } => {
                                    this.scan_status = format!("正在扫描 … 已处理 {} 张", processed);
                                }
                                ScanProgress::Finished { stats } => {
                                    this.selected.clear();
                                    this.reload_all();
                                    this.scan_status = format!(
                                        "扫描完成：新增 {} 张，更新 {} 张，跳过 {} 张，清理 {} 张，耗时 {} ms",
                                        stats.photos_added, stats.photos_updated,
                                        stats.photos_skipped, stats.photos_removed, stats.duration_ms
                                    );
                                    this.is_scanning = false;
                                    this.progress_rx = None;
                                    this.last_error = None;
                                    done = true;
                                    // 只有索引真正变化时才重跑分析，避免重复扫描触发 O(n²) 全量重算。
                                    if stats.photos_added > 0
                                        || stats.photos_updated > 0
                                        || stats.photos_removed > 0
                                    {
                                        this.start_analysis(cx);
                                    }
                                }
                                ScanProgress::Failed { message } => {
                                    this.scan_status = format!("扫描失败：{message}");
                                    this.last_error = Some(format!("扫描失败：{message}"));
                                    this.is_scanning = false;
                                    this.progress_rx = None;
                                    done = true;
                                }
                            }
                        }
                        cx.notify();
                        done
                    })
                    .ok()
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        }).detach();
    }

    /// 启动后台分析：遍历索引计算模糊/截图/相似组，写库并回传进度。
    /// 扫描完成后会自动调用；也可在整理页手动重新分析。
    pub fn start_analysis(&mut self, cx: &mut Context<Self>) {
        if self.is_analyzing {
            self.pending_analysis = true;
            return;
        }
        self.pending_analysis = false;
        self.is_analyzing = true;
        self.analyze_status = "正在分析照片 …".to_string();

        let (tx, rx) = std::sync::mpsc::channel::<AnalyzeProgress>();
        self.analyze_rx = Some(rx);
        let paths = self.paths.clone();

        // 后台线程：自己打开 DB 连接运行分析（Connection 不 Send）。
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<AnalyzeReport> {
                let conn = crate::core::db::open(&paths.db_path)?;
                crate::core::analyze::analyze_library(&conn, Some(&tx))
            })();
            if let Err(e) = result {
                let _ = tx.send(AnalyzeProgress::Failed { message: e.to_string() });
            }
        });

        // UI 侧：与扫描一致，每轮排空 channel，避免 Finished 被进度消息长期挤在队尾。
        cx.spawn(async move |entity, app: &mut AsyncApp| {
            loop {
                gpui::background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;

                let messages = match entity
                    .update(app, |this: &mut Self, _cx| {
                        let Some(rx) = &this.analyze_rx else {
                            return Vec::new();
                        };
                        let mut out = Vec::new();
                        let mut disconnected = false;
                        loop {
                            match rx.try_recv() {
                                Ok(p) => out.push(p),
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    disconnected = true;
                                    break;
                                }
                            }
                        }
                        let has_terminal = out.iter().any(|m| {
                            matches!(
                                m,
                                AnalyzeProgress::Finished { .. }
                                    | AnalyzeProgress::Failed { .. }
                            )
                        });
                        if disconnected && !has_terminal {
                            out.push(AnalyzeProgress::Failed {
                                message: "分析线程意外退出".to_string(),
                            });
                        }
                        out
                    })
                    .ok()
                {
                    Some(v) => v,
                    None => break,
                };
                if messages.is_empty() {
                    continue;
                }

                let done = entity
                    .update(app, |this: &mut Self, cx| {
                        let mut done = false;
                        for p in messages.iter().cloned() {
                            match p {
                                AnalyzeProgress::Started { total } => {
                                    this.analyze_status = format!("正在分析 … 共 {total} 张");
                                }
                                AnalyzeProgress::Analyzing { processed, total } => {
                                    this.analyze_status =
                                        format!("正在分析 … {processed}/{total} 张");
                                }
                                AnalyzeProgress::Finished { report } => {
                                    // 分析结果会写回 photos，详情弹窗也依赖这些字段，必须整体刷新。
                                    this.reload_all();
                                    this.analyze_status = format!(
                                        "分析完成：模糊 {} 张，截图 {} 张，相似组 {} 组，失败 {} 张，耗时 {} ms",
                                        report.blurry_count, report.screenshot_count,
                                        report.similar_groups, report.photos_failed,
                                        report.duration_ms
                                    );
                                    this.is_analyzing = false;
                                    this.analyze_rx = None;
                                    this.last_error = None;
                                    done = true;
                                }
                                AnalyzeProgress::Failed { message } => {
                                    this.analyze_status = format!("分析失败：{message}");
                                    this.last_error = Some(format!("分析失败：{message}"));
                                    this.is_analyzing = false;
                                    this.analyze_rx = None;
                                    done = true;
                                }
                            }
                        }
                        if done && this.pending_analysis {
                            this.pending_analysis = false;
                            this.start_analysis(cx);
                        }
                        cx.notify();
                        done
                    })
                    .ok()
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        }).detach();
    }
}

// ===== 根组件 =====
pub struct AppRoot {
    app: Entity<AppState>,
    /// 相册库搜索框与重命名前缀输入框（首次 render 时创建，需要 Window 上下文）。
    search_input: Option<Entity<InputState>>,
    prefix_input: Option<Entity<InputState>>,
    /// Subscription 必须持有，drop 即取消订阅。
    _subscriptions: Vec<gpui::Subscription>,
}

impl AppRoot {
    pub fn new(app: Entity<AppState>) -> Self {
        Self {
            app,
            search_input: None,
            prefix_input: None,
            _subscriptions: Vec::new(),
        }
    }

    /// 首次渲染时创建输入状态：InputState::new 需要 Window，AppRoot::new 阶段拿不到。
    /// 输入变化同步到 AppState，保证预览/过滤实时刷新。
    fn ensure_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("搜索文件名 / 地点 / 标签")
        });
        let prefix = cx.new(|cx| {
            InputState::new(window, cx).placeholder("重命名前缀（可选）")
        });

        let app_for_search = self.app.clone();
        let s = search.clone();
        self._subscriptions.push(cx.subscribe(
            &search,
            move |_this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = s.read(cx).value().to_string();
                    app_for_search.update(cx, |st, cx| {
                        st.data.keyword = v;
                        cx.notify();
                    });
                }
            },
        ));

        let app_for_prefix = self.app.clone();
        let p = prefix.clone();
        self._subscriptions.push(cx.subscribe(
            &prefix,
            move |_this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    let v = p.read(cx).value().to_string();
                    app_for_prefix.update(cx, |st, cx| {
                        st.rename_prefix = v;
                        cx.notify();
                    });
                }
            },
        ));

        self.search_input = Some(search);
        self.prefix_input = Some(prefix);
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_inputs(window, cx);
        let state = self.app.read(cx);
        let search_input = self.search_input.clone();
        let prefix_input = self.prefix_input.clone();

        // 自绘标题栏（红=关闭 / 黄=最小化 / 绿=最大化还原），系统标题栏已在 main.rs 隐藏。
        let titlebar = crate::ui::window_titlebar("Akashic · 相册整理师", "本地 · 离线");

        let sidebar = self.sidebar(state);
        // 全局错误横幅：索引/后台任务失败时，任意页面都能看到，而不是只在首页/设置页。
        let error_banner: AnyElement = if let Some(err) =
            state.load_error.as_ref().or(state.last_error.as_ref())
        {
            div()
                .id("global-error-banner")
                .mb(px(12.))
                .px(px(14.))
                .py(px(10.))
                .rounded_md()
                .bg(rgba(0xE0, 0x54, 0x4C, 0.10))
                .text_size(px(12.))
                .text_color(col(DANGER))
                .child(format!("⚠ {err}"))
                .into_any_element()
        } else {
            div().id("global-error-none").into_any_element()
        };
        let content = div()
            .id("content")
            .flex_1()
            .min_h_0()
            // 关键：gpui 的 div() 默认是 Display::Block，子元素纵向堆叠且高度自适应。
            // 不显式开 flex 的话，需要确定高度的页面拿不到高度。
            .flex()
            .flex_col()
            .bg(col(BG_WARM))
            .px(px(24.))
            .py(px(20.))
            .overflow_y_scroll()
            .child(error_banner)
            .child(self.render_page(state, search_input, prefix_input));

        // 窗口内容直接铺满客户区：此前外层 0xE8E4DE 底色 + 圆角卡片会形成一圈“额外边框”。
        div()
            .id("window")
            .size_full()
            .flex().flex_col()
            .bg(col(BG_WHITE))
            .child(titlebar)
            .child(
                div()
                    .id("mainlayout")
                    .flex_1()
                    .min_h_0()
                    .flex().flex_row()
                    .child(sidebar)
                    .child(content),
            )
    }
}

/// 供组合根创建初始状态（main.rs 使用）。
pub fn new_app_state(cx: &mut App, paths: Paths) -> Entity<AppState> {
    AppState::new(cx, paths)
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
        // 待处理项总数（替换原硬编码的模拟徽标 12）
        let analyze_total = state.analyze_data.groups.len()
            + state.analyze_data.blurry.len()
            + state.analyze_data.screenshots.len()
            + state.analyze_data.large.len();
        let nav = |page: Page, icon: IconName, label: &'static str, badge: Option<String>| {
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
                            .child(SharedString::from(badge_text)),
                    ])
                })
                .on_click(move |_ev, _w, cx| {
                    let _ = app.update(cx, |s, cx| {
                        s.page = page;
                        // 进入归档页时重算预览：扫描新增照片或改变选中后，旧计划会过期。
                        if matches!(page, Page::Archive) {
                            s.refresh_archive_plan();
                        }
                        cx.notify();
                    });
                });
            btn.into_any_element()
        };

        div()
            .id("sidebar")
            .w(px(200.))
            .flex_none()
            .min_h_0()
            .flex().flex_col()
            .bg(col(BG_SIDEBAR))
            .pt(px(4.))
            .child(nav(Page::Home, IconName::LayoutDashboard, "首页", None))
            .child(nav(
                Page::Organize,
                IconName::Settings2,
                "智能整理",
                Some(format!("{}", analyze_total)),
            ))
            .child(nav(
                Page::Library,
                IconName::GalleryVerticalEnd,
                "相册库",
                Some(format!("{}", state.data.total_count)),
            ))
            .child(nav(Page::Rename, IconName::CaseSensitive, "重命名", None))
            .child(nav(Page::Archive, IconName::FolderOpen, "智能归档", None))
            .child(nav(
                Page::Trash,
                IconName::Delete,
                "回收站",
                if state.ops_data.trash_count > 0 {
                    Some(format!("{}", state.ops_data.trash_count))
                } else {
                    None
                },
            ))
            .child(div().id("sidebar-divider").h(px(1.)).my(px(12.)).mx(px(20.)).bg(rgba(255, 255, 255, 0.06)))
            .child(nav(Page::Settings, IconName::Settings, "设置", None))
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

    fn render_page(
        &self,
        state: &AppState,
        search_input: Option<Entity<InputState>>,
        prefix_input: Option<Entity<InputState>>,
    ) -> impl IntoElement + 'static {
        match state.page {
            Page::Home => self.page_home(state).into_any_element(),
            Page::Organize => self.page_organize(state).into_any_element(),
            Page::Library => self.page_library(state, search_input).into_any_element(),
            Page::Rename => self.page_rename(state, prefix_input).into_any_element(),
            Page::Archive => self.page_archive(state).into_any_element(),
            Page::Trash => self.page_trash(state).into_any_element(),
            Page::Settings => self.page_settings(state).into_any_element(),
        }
    }
}

impl AppRoot {
    // ===== 首页 =====
    // ===== 首页 =====
    fn page_home(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        // 一次性提取真实统计到 owned 值，避免长时间持有 state 引用造成借用冲突
        let total = state.data.total_count;
        let total_bytes = state.data.total_bytes;
        let root_count = state.data.root_count;
        let size_text = human_size(total_bytes);
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

        // 错误态由 AppRoot 的全局横幅统一展示，这里只保留后台任务进度。
        // 扫描优先于分析，避免先分析过一次后就永远看不到扫描进度。
        let banner: AnyElement = if state.is_scanning || state.is_analyzing {
            let text = if state.is_scanning {
                state.scan_status.clone()
            } else {
                state.analyze_status.clone()
            };
            div()
                .id("home-banner")
                .mb(px(14.))
                .px(px(14.))
                .py(px(10.))
                .rounded_md()
                .bg(col(GOLD_PALE))
                .text_size(px(12.))
                .text_color(col(PRIMARY))
                .child(format!("⏳ {text}"))
                .into_any_element()
        } else {
            div().id("home-banner-none").into_any_element()
        };

        // 整理进度：真实分析结果 + 后台分析状态
        let a = &state.analyze_data;
        let progress_text = if state.is_analyzing {
            state.analyze_status.clone()
        } else if !state.analyze_status.is_empty() {
            state.analyze_status.clone()
        } else if a.is_ready() {
            format!("已完成 {} 张照片的分析 · 结果来自本地索引", a.analyzed_count)
        } else {
            "尚未分析 · 扫描目录后自动开始".to_string()
        };
        let progress_items: Vec<(String, &'static str)> = vec![
            (format!("{}", a.groups.len()), "相似组"),
            (format!("{}", a.blurry.len()), "疑似模糊"),
            (format!("{}", a.screenshots.len()), "截图"),
            (format!("{}", a.large.len()), "大文件"),
        ];
        let progress_row = div()
            .id("home-progress")
            .flex().flex_row()
            .gap(px(10.))
            .children(
                progress_items
                    .into_iter()
                    .enumerate()
                    .map(|(i, (v, label))| {
                        div()
                            .id(("home-progress-item", i))
                            .flex_1()
                            .bg(col(BG_WARM))
                            .rounded_md()
                            .px(px(12.))
                            .py(px(10.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(col(GOLD))
                                    .child(v),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(col(TEXT_SECONDARY))
                                    .child(label.to_owned()),
                            )
                    })
                    .collect::<Vec<_>>(),
            );

        // 最近整理记录：来自 op_log 的真实批量操作（空态给出引导）
        let recent_ops = state.ops_data.recent_ops.clone();
        let records_body: AnyElement = if recent_ops.is_empty() {
            div()
                .id("home-records-empty")
                .flex().flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .py(px(24.))
                .child(Icon::new(IconName::Info).size(px(32.)).text_color(col(BORDER_LIGHT)))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(col(TEXT_SECONDARY))
                        .child("还没有整理记录"),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(col(TEXT_LIGHT))
                        .child("扫描目录并完成一次整理后，操作会记录在这里"),
                )
                .into_any_element()
        } else {
            div()
                .id("home-records-list")
                .flex().flex_col()
                .gap(px(6.))
                .children(
                    recent_ops
                        .iter()
                        .take(6)
                        .enumerate()
                        .map(|(i, op)| {
                            div()
                                .id(("home-record", i))
                                .flex().flex_row()
                                .items_center()
                                .gap(px(12.))
                                .px(px(12.))
                                .py(px(9.))
                                .bg(col(BG_WARM))
                                .rounded_md()
                                .child(
                                    div()
                                        .w(px(132.))
                                        .flex_none()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(col(PRIMARY))
                                        .child(op.kind.label().to_owned()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(12.))
                                        .text_color(col(GOLD))
                                        .child(format!("{} 张", op.item_count)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(11.))
                                        .text_color(col(TEXT_SECONDARY))
                                        .child(truncate(&op.detail, 42)),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(11.))
                                        .text_color(col(TEXT_LIGHT))
                                        .child(format_millis(op.created_at)),
                                )
                        })
                        .collect::<Vec<_>>(),
                )
                .into_any_element()
        };

        let app2 = app.clone();
        let organize_btn = Button::new("home-organize")
            .primary()
            .icon(IconName::Settings2)
            .label("开始整理")
            .on_click(move |_ev, _w, cx| {
                app2.update(cx, |s, cx| {
                    s.page = Page::Organize;
                    cx.notify();
                });
            });
        let app3 = app.clone();
        let library_btn = Button::new("home-library")
            .outline()
            .icon(IconName::FolderOpen)
            .label("选择目录")
            .on_click(move |_ev, _w, cx| {
                // 与相册库工具栏一致：直接弹原生目录对话框并建立索引，
                // 而不是只跳页让用户再找一次按钮。
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("选择照片目录")
                    .pick_folder()
                {
                    let path = folder.to_path_buf();
                    app3.update(cx, |s, cx| {
                        s.page = Page::Library;
                        s.start_scan(path, cx);
                        cx.notify();
                    });
                }
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
                    .child(
                        div()
                            .flex().flex_row()
                            .gap(px(8.))
                            .child(library_btn)
                            .child(organize_btn),
                    ),
            )
            .child(banner)
            .child(
                div()
                    .id("home-stats")
                    .flex().flex_row()
                    .gap(px(14.))
                    .children(stat_cards.collect::<Vec<_>>()),
            )
            .child(
                div()
                    .id("home-progress-card")
                    .bg(col(BG_WHITE))
                    .rounded_lg()
                    .px(px(20.))
                    .py(px(16.))
                    .shadow_sm()
                    .mt(px(18.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(col(TEXT_SECONDARY))
                            .mb(px(6.))
                            .child("🧭 整理进度"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(TEXT_LIGHT))
                            .mb(px(10.))
                            .child(progress_text),
                    )
                    .child(progress_row),
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
                    .child(records_body),
            )
    }

    // ===== 智能整理页 =====
    fn page_organize(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        let badges = state.analyze_data.badges();
        // 前 4 个为 M2 分析工具（进入结果列表）；后 2 个跳转到 M3/M4 的真实页面。
        let tools = vec![
            (IconName::Copy, "相似照片", "连拍 & 重复角度", badges[0].clone(), false, Some(OrganizeTool::Similar), None),
            (IconName::EyeOff, "模糊检测", "手抖 / 失焦", badges[1].clone(), false, Some(OrganizeTool::Blurry), None),
            (IconName::Frame, "截图筛选", "屏幕录制 · 截屏", badges[2].clone(), false, Some(OrganizeTool::Screenshot), None),
            (IconName::File, "大文件", "按大小排序", badges[3].clone(), false, Some(OrganizeTool::Large), None),
            (IconName::CaseSensitive, "智能重命名", "统一命名风格", "已就绪".to_string(), false, None, Some(Page::Rename)),
            (IconName::FolderOpen, "智能归档", "自动归入目录", "已就绪".to_string(), false, None, Some(Page::Archive)),
        ];
        let tool_cards = tools.into_iter().enumerate().map(|(ti, (icon, name, desc, badge, star, tool, page))| {
            let app = app.clone();
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
                .on_click(move |_ev, _w, cx| {
                    if let Some(p) = page {
                        app.update(cx, |s, cx| {
                            s.page = p;
                            cx.notify();
                        });
                    } else if let Some(t) = tool {
                        app.update(cx, |s, cx| {
                            s.active_tool = Some(t);
                            cx.notify();
                        });
                    }
                })
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

        // 顶部状态：优先显示分析进度，其次扫描进度。
        let status = if state.is_analyzing || !state.analyze_status.is_empty() {
            state.analyze_status.clone()
        } else {
            state.scan_status.clone()
        };
        let app_analyze = app.clone();
        let header = div()
            .id("organize-header")
            .flex().flex_row()
            .justify_between()
            .items_center()
            .mb(px(12.))
            .child(page_title("🔍 智能整理", "选择工具开始清理"))
            .child(
                Button::new("organize-analyze")
                    .outline()
                    .icon(IconName::Redo)
                    .label("重新分析")
                    .on_click(move |_ev, _w, cx| {
                        app_analyze.update(cx, |s, cx| {
                            s.start_analysis(cx);
                            cx.notify();
                        });
                    }),
            );

        // 选中工具时展示结果列表，否则展示工具卡片总览。
        if let Some(tool) = state.active_tool {
            let app_back = app.clone();
            return div()
                .id("page-organize")
                .flex().flex_col()
                .child(header)
                .child(
                    div()
                        .id("analyze-status")
                        .text_size(px(12.))
                        .text_color(col(TEXT_SECONDARY))
                        .mb(px(10.))
                        .child(status),
                )
                .child(
                    Button::new("organize-back")
                        .ghost()
                        .icon(IconName::ArrowLeft)
                        .label("返回工具列表")
                        .on_click(move |_ev, _w, cx| {
                            app_back.update(cx, |s, cx| {
                                s.active_tool = None;
                                cx.notify();
                            });
                        }),
                )
                .child(self.render_tool_result(tool, state))
                .into_any_element();
        }

        div()
            .id("page-organize")
            .flex().flex_col()
            .child(header)
            .child(
                div()
                    .id("analyze-status")
                    .text_size(px(12.))
                    .text_color(col(TEXT_SECONDARY))
                    .mb(px(10.))
                    .child(status),
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
            .into_any_element()
    }

    /// 渲染某个分析工具的结果列表（相似组按组展示，其余按照片展示）。
    fn render_tool_result(&self, tool: OrganizeTool, state: &AppState) -> impl IntoElement + 'static {
        let d = &state.analyze_data;
        if !d.is_ready() {
            return div()
                .id("analyze-empty")
                .flex_1()
                .flex().flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(Icon::new(IconName::EyeOff).size(px(40.)).text_color(col(TEXT_LIGHT)))
                .child(div().text_size(px(14.)).text_color(col(TEXT_SECONDARY))
                    .child("尚无分析结果，请先扫描目录（扫描完成后会自动分析）"))
                .into_any_element();
        }

        match tool {
            OrganizeTool::Similar => self.render_similar_groups(state).into_any_element(),
            OrganizeTool::Blurry => self.render_photo_list(&d.blurry, "模糊照片", "按模糊程度排序", state.paths.clone()).into_any_element(),
            OrganizeTool::Screenshot => self.render_photo_list(&d.screenshots, "截图", "按拍摄时间排序", state.paths.clone()).into_any_element(),
            OrganizeTool::Large => self.render_photo_list(&d.large, "大文件", "按文件大小排序", state.paths.clone()).into_any_element(),
        }
    }

    /// 通用照片结果列表（模糊/截图/大文件）。
    fn render_photo_list(&self, photos: &[Photo], title: &str, sub: &str, paths: Paths) -> impl IntoElement + 'static {
        let tiles: Vec<_> = photos.iter().enumerate().map(|(i, p)| {
            let dialog_photo = p.clone();
            let editor_paths = paths.clone();
            div()
                .id(("resultitem", i))
                .w(px(180.))
                .bg(col(BG_WHITE))
                .rounded_lg()
                .overflow_hidden()
                .shadow_sm()
                .cursor_pointer()
                .on_click(move |_ev, window, cx| {
                    open_photo_editor(dialog_photo.clone(), editor_paths.clone(), cx);
                    let _ = window;
                })
                .child(
                    div()
                        .id(("resultthumb", i))
                        .h(px(110.))
                        .w_full()
                        .overflow_hidden()
                        .child(photo_thumb(p)),
                )
                .child(
                    div()
                        .id(("resultinfo", i))
                        .px(px(10.))
                        .py(px(8.))
                        .child(div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD)
                            .child(p.file_name.clone()))
                        .child(div().text_size(px(10.)).mt(px(2.)).text_color(col(TEXT_SECONDARY))
                            .child(format!("{} · {}", p.file_size_label(), p.date_label())))
                        .when(p.blur_score.is_some(), |d| d.child(
                            div().text_size(px(10.)).mt(px(2.)).text_color(col(TEXT_LIGHT))
                                .child(format!("模糊分 {:.0}", p.blur_score.unwrap())))),
                )
        }).collect();

        div()
            .id("photo-result")
            .flex_1()
            .flex().flex_col()
            .child(page_title(title.to_string(), sub.to_string()))
            .child(div().text_size(px(12.)).text_color(col(TEXT_SECONDARY)).mb(px(10.))
                .child(format!("共 {} 张", photos.len())))
            .child(
                div()
                    .id("photo-result-grid")
                    .flex().flex_row().flex_wrap()
                    .gap(px(12.))
                    .children(tiles),
            )
    }

    /// 相似组结果：每组一张代表卡片，显示成员数。
    fn render_similar_groups(&self, state: &AppState) -> impl IntoElement + 'static {
        let groups = &state.analyze_data.groups;
        let editor_paths = state.paths.clone();
        let cards: Vec<_> = groups.iter().enumerate().map(|(gi, g)| {
            let rep = g.members.first();
            let name = rep.map(|p| p.file_name.clone()).unwrap_or_default();
            let hue = rep.map(|p| stable_hue(&p.path.to_string_lossy())).unwrap_or(0.0);
            let dialog_photo = rep.cloned();
            let click_paths = editor_paths.clone();
            let thumb_el = rep
                .map(photo_thumb)
                .unwrap_or_else(|| photo_placeholder(hue, "", &name));
            div()
                .id(("group", gi))
                .w(px(220.))
                .bg(col(BG_WHITE))
                .rounded_lg()
                .overflow_hidden()
                .shadow_sm()
                .cursor_pointer()
                .on_click(move |_ev, window, cx| {
                    if let Some(p) = dialog_photo.clone() {
                        open_photo_editor(p, click_paths.clone(), cx);
                    }
                    let _ = window;
                })
                .child(
                    div().id(("groupthumb", gi)).h(px(120.)).w_full().overflow_hidden()
                        .child(thumb_el),
                )
                .child(
                    div().id(("groupinfo", gi)).px(px(12.)).py(px(10.))
                        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{} 张相似", g.members.len())))
                        .child(div().text_size(px(10.)).mt(px(2.)).text_color(col(TEXT_SECONDARY))
                            .child(name)),
                )
        }).collect();

        div()
            .id("similar-result")
            .flex_1()
            .flex().flex_col()
            .child(page_title("相似照片".to_string(), format!("{} 组", groups.len())))
            .child(
                div().id("similar-grid").flex().flex_row().flex_wrap().gap(px(12.))
                    .children(cards),
            )
    }

    // ===== 相册库页 =====
    fn page_library(&self, state: &AppState, search_input: Option<Entity<InputState>>) -> impl IntoElement + 'static {
        let app = &self.app;

        // 搜索框：输入变化由 AppRoot 的订阅同步到 data.keyword，过滤即时生效。
        let search_box: AnyElement = match search_input {
            Some(st) => div()
                .id("grid-search")
                .w(px(230.))
                .flex_none()
                .child(Input::new(&st))
                .into_any_element(),
            None => div().id("grid-search-none").into_any_element(),
        };

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

        // 相册库网格：与整理页 render_photo_list 完全相同的渲染路径（普通 div + flex_wrap + 固定宽 tile）。
        // 曾尝试 uniform_list 虚拟滚动，但其 item 测量在此布局下把缩略图压成竖线且反复修不好；
        // 正确性优先，回退到已验证可用的普通网格。5000 张量级的性能权衡留待后续单独处理。
        let photos: Vec<Photo> = state
            .data
            .visible_photos()
            .into_iter()
            .cloned()
            .collect();
        let selected = state.selected.clone();
        let app_for_list = app.clone();
        let editor_paths = state.paths.clone();
        let total_visible = photos.len();

        // 空态：未扫描时给出引导
        let grid_body: AnyElement = if photos.is_empty() {
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
            let tiles: Vec<AnyElement> = photos
                .iter()
                .enumerate()
                .map(|(idx, photo)| {
                    let photo_id = photo.id;
                    let is_selected = selected.contains(&photo_id);
                    let location = photo.location.clone().unwrap_or_default();
                    let dialog_photo = photo.clone();
                    let tile_app = app_for_list.clone();
                    let tile_paths = editor_paths.clone();
                    div()
                        .id(("photo", idx))
                        .w(px(GRID_TILE_WIDTH))
                        .relative()
                        .overflow_hidden()
                        .bg(photo_block_bg(stable_hue(&photo.path.to_string_lossy())))
                        .border_2()
                        .when(is_selected, |d| d.border_color(col(GOLD)))
                        .when(!is_selected, |d| d.border_color(transparent_black()))
                        .cursor_pointer()
                        .on_click(move |ev, _window, cx| {
                            if ev.modifiers().control || ev.modifiers().platform {
                                tile_app.update(cx, |s, cx| {
                                    if !s.selected.insert(photo_id) {
                                        s.selected.remove(&photo_id);
                                    }
                                    cx.notify();
                                });
                            } else {
                                open_photo_editor(dialog_photo.clone(), tile_paths.clone(), cx);
                            }
                        })
                        .child(
                            div()
                                .id(("photothumb", idx))
                                .h(px(GRID_TILE_HEIGHT))
                                .w_full()
                                .overflow_hidden()
                                .child(photo_thumb(photo)),
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
                        .into_any_element()
                })
                .collect();
            div()
                .id("photo-grid-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    div()
                        .id("photo-grid")
                        .flex().flex_row().flex_wrap()
                        .gap(px(12.))
                        .children(tiles),
                )
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
                            // 默认 Block 会让「共 / 39 / 张照片」竖着堆叠，显式开 flex 横排。
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(4.))
                            .flex_none()
                            .text_color(col(TEXT_SECONDARY))
                            .child("共 ")
                            .child(
                                div()
                                    .id("grid-count")
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(col(PRIMARY))
                                    .child(format!("{}", total_visible)),
                            )
                            .child(" 张照片"),
                    )
                    .child(search_box)
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
                        cx.notify();
                    });
                }
            });

        // 选中照片后的批量操作按钮（重命名前往重命名页确认模板，删除直接移入回收站）
        let app_rename = app.clone();
        let rename_nav = Button::new("nav-rename")
            .outline()
            .icon(IconName::CaseSensitive)
            .label("重命名")
            .on_click(move |_ev, _w, cx| {
                app_rename.update(cx, |s, cx| {
                    s.page = Page::Rename;
                    cx.notify();
                });
            });
        let app_del = app.clone();
        let has_selection = !state.selected.is_empty();
        let delete_btn = Button::new("batch-delete")
            .outline()
            .icon(IconName::Delete)
            .label(if has_selection { "删除到回收站" } else { "删除" })
            .text_color(col(if has_selection { DANGER } else { TEXT_SECONDARY }))
            .on_click(move |_ev, _w, cx| {
                app_del.update(cx, |s, cx| {
                    s.batch_trash();
                    cx.notify();
                });
            });

        // 批量操作状态提示（删除/重命名/撤销结果）
        let ops_status = if state.ops_status.is_empty() {
            div().id("ops-status-none")
        } else {
            div()
                .id("ops-status")
                .mb(px(8.))
                .px(px(12.))
                .py(px(6.))
                .rounded_md()
                .bg(col(GOLD_PALE))
                .text_size(px(12.))
                .text_color(col(PRIMARY))
                .child(state.ops_status.clone())
        };

        div()
            .id("page-library")
            // flex_1：填满 content 剩余高度，给 library-layout → uniform_list 一条完整的
            // 确定高度链（content → page-library → library-layout → grid_panel → 列表）。
            .flex_1()
            .min_h_0()
            .flex().flex_col()
            .child(status_bar)
            .child(ops_status)
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
                            rename_nav,
                            delete_btn,
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

    // ===== 重命名页（M3） =====
    fn page_rename(&self, state: &AppState, prefix_input: Option<Entity<InputState>>) -> impl IntoElement + 'static {
        let app = &self.app;
        let ids = state.selected_photo_ids();
        let count = ids.len();

        // 前缀输入框：让 `{前缀}` 占位符真正生效（此前字段永远为空，示例名无法复现）。
        let prefix_box: AnyElement = match prefix_input {
            Some(st) => div()
                .id("rename-prefix-input")
                .w(px(260.))
                .flex_none()
                .child(Input::new(&st))
                .into_any_element(),
            None => div().id("rename-prefix-input-none").into_any_element(),
        };

        // 预设模板（点击即选用，避免文本输入框在 dialog 中的 window 生命周期复杂度）
        let presets: Vec<(&str, &str, &str)> = vec![
            ("{前缀}_{日期}_{序号:3}", "前缀+日期+序号", "日本_20240815_001.jpg"),
            ("{前缀}_{序号:3}", "前缀+序号", "日本_001.jpg"),
            ("{日期}_{时间}_{序号:3}", "日期+时间+序号", "20240815_094523_001.jpg"),
            ("{前缀}_{地点}_{序号:3}", "前缀+地点+序号", "日本_富士山_001.jpg"),
            ("{前缀}_{标签}_{序号:3}", "前缀+标签+序号", "日本_旅行_001.jpg"),
        ];

        // 实时预览：用当前 pattern/prefix 计算前 5 条
        let preview_items: Vec<(String, String)> = if count > 0 {
            crate::core::db::open(&state.paths.db_path)
                .ok()
                .and_then(|conn| {
                    ops_rename::preview(&conn, &ids, &state.rename_pattern, &state.rename_prefix, 5).ok()
                })
                .map(|pv| {
                    pv.items
                        .into_iter()
                        .map(|it| (it.old_name, it.new_name))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let preset_cards = presets
            .into_iter()
            .enumerate()
            .map(|(i, (pattern, label, example))| {
                let app = app.clone();
                let pat = pattern.to_string();
                let active = state.rename_pattern == pattern;
                div()
                    .id(("rename-preset", i))
                    .flex_1()
                    .bg(col(if active { GOLD_PALE } else { BG_WARM }))
                    .rounded_md()
                    .px(px(12.))
                    .py(px(10.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(col(if active { GOLD } else { BORDER_LIGHT }))
                    .on_click(move |_ev, _w, cx| {
                        let pat = pat.clone();
                        app.update(cx, |s, cx| {
                            s.rename_pattern = pat;
                            cx.notify();
                        });
                    })
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(label.to_string()))
                    .child(div().text_size(px(10.)).mt(px(4.)).text_color(col(TEXT_LIGHT)).child(example.to_string()))
            })
            .collect::<Vec<_>>();

        let preview_rows = preview_items
            .into_iter()
            .enumerate()
            .map(|(i, (old, new))| {
                div()
                    .id(("rename-preview", i))
                    .flex().flex_row().items_center()
                    .gap(px(10.))
                    .px(px(12.)).py(px(7.))
                    .bg(col(BG_WARM))
                    .rounded_md()
                    .text_size(px(12.))
                    .child(div().w(px(260.)).text_color(col(TEXT_SECONDARY)).child(old))
                    .child(Icon::new(IconName::ArrowRight).size_3().text_color(col(GOLD)))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(new))
            })
            .collect::<Vec<_>>();

        let app_exec = app.clone();
        let exec_btn = Button::new("rename-exec")
            .primary()
            .icon(IconName::CaseSensitive)
            .label(format!("执行重命名（{} 张）", count))
            .disabled(count == 0)
            .on_click(move |_ev, _w, cx| {
                app_exec.update(cx, |s, cx| {
                    s.batch_rename();
                    cx.notify();
                });
            });
        let app_undo = app.clone();
        let undo_btn = Button::new("rename-undo")
            .outline()
            .icon(IconName::Undo)
            .label("撤销上次重命名")
            .disabled(state.last_rename_op.is_none())
            .on_click(move |_ev, _w, cx| {
                app_undo.update(cx, |s, cx| {
                    s.undo_rename();
                    cx.notify();
                });
            });

        let status = if state.ops_status.is_empty() {
            format!("从相册库 Ctrl+点按选择照片后，这里会显示重命名预览。当前选中 {} 张。", count)
        } else {
            state.ops_status.clone()
        };

        div()
            .id("page-rename")
            .flex().flex_col()
            .child(page_title("✏️ 批量重命名", "选择命名模板，预览后执行 · 可一次撤销"))
            .child(div().id("rename-status").text_size(px(12.)).text_color(col(TEXT_SECONDARY))
                .my(px(10.)).child(status))
            .child(
                div()
                    .id("rename-prefix-row")
                    .flex().flex_row().items_center().gap(px(10.))
                    .mb(px(12.))
                    .child(div().text_size(px(12.)).text_color(col(TEXT_SECONDARY)).child("前缀："))
                    .child(prefix_box),
            )
            .child(div().id("rename-presets").flex().flex_row().gap(px(10.)).mb(px(16.)).children(preset_cards))
            .child(
                div().id("rename-preview-panel").flex_1().bg(col(BG_WHITE)).rounded_lg()
                    .p(px(18.)).shadow_sm().flex().flex_col()
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD)
                        .text_color(col(TEXT_SECONDARY)).mb(px(10.)).child("📐 命名预览（前 5 张）"))
                    .child(if preview_rows.is_empty() {
                        div().id("rename-preview-empty").flex_1().flex().flex_col()
                            .items_center().justify_center().gap(px(8.))
                            .child(Icon::new(IconName::CaseSensitive).size(px(40.)).text_color(col(BORDER_LIGHT)))
                            .child(div().text_size(px(13.)).text_color(col(TEXT_LIGHT))
                                .child("请先在相册库选择要重命名的照片"))
                            .into_any_element()
                    } else {
                        div().id("rename-preview-list").flex().flex_col().gap(px(6.))
                            .children(preview_rows).into_any_element()
                    })
            )
            .child(div().id("rename-actions").flex().flex_row().gap(px(10.)).mt(px(14.))
                .justify_end()
                .child(undo_btn)
                .child(exec_btn))
    }

    // ===== 智能归档页（M4） =====
    fn page_archive(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        let plan = state.archive_plan.clone();
        let (total_files, total_bytes, conflict_count, dest_label, rule_label, mode_label) =
            match &plan {
                Some(p) => (
                    p.total_files,
                    p.total_bytes,
                    p.conflict_count,
                    p.dest_root.to_string_lossy().to_string(),
                    p.rule.label().to_string(),
                    p.mode.label().to_string(),
                ),
                None => (
                    0usize,
                    0i64,
                    0usize,
                    "（未生成预览）".to_string(),
                    state.archive_rule.label().to_string(),
                    state.archive_mode.label().to_string(),
                ),
            };

        // ① 规则预设
        let rule_cards = ArchiveRule::ALL
            .iter()
            .enumerate()
            .map(|(i, rule)| {
                let app = app.clone();
                let r = *rule;
                let active = state.archive_rule == r;
                div()
                    .id(("arc-rule", i))
                    .flex_1()
                    .bg(col(if active { GOLD_PALE } else { BG_WARM }))
                    .rounded_md()
                    .px(px(12.))
                    .py(px(10.))
                    .cursor_pointer()
                    .border_1()
                    .border_color(col(if active { GOLD } else { BORDER_LIGHT }))
                    .on_click(move |_ev, _w, cx| {
                        app.update(cx, |s, cx| {
                            s.archive_rule = r;
                            s.persist_archive_settings();
                            s.refresh_archive_plan();
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(r.label().to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .mt(px(4.))
                            .text_color(col(TEXT_LIGHT))
                            .child(r.example().to_string()),
                    )
            })
            .collect::<Vec<_>>();

        // ② 模式（移动 / 复制）
        let app_move = app.clone();
        let move_btn = Button::new("arc-mode-move")
            .when(state.archive_mode == ArchiveMode::Move, |b| b.primary())
            .when(state.archive_mode != ArchiveMode::Move, |b| b.outline())
            .label("移动（整理原目录）")
            .on_click(move |_ev, _w, cx| {
                app_move.update(cx, |s, cx| {
                    s.archive_mode = ArchiveMode::Move;
                    s.persist_archive_settings();
                    s.refresh_archive_plan();
                    cx.notify();
                });
            });
        let app_copy = app.clone();
        let copy_btn = Button::new("arc-mode-copy")
            .when(state.archive_mode == ArchiveMode::Copy, |b| b.primary())
            .when(state.archive_mode != ArchiveMode::Copy, |b| b.outline())
            .label("复制（保留原文件）")
            .on_click(move |_ev, _w, cx| {
                app_copy.update(cx, |s, cx| {
                    s.archive_mode = ArchiveMode::Copy;
                    s.persist_archive_settings();
                    s.refresh_archive_plan();
                    cx.notify();
                });
            });

        // ③ 范围（全部可见 / 仅选中）
        let scope_text = if state.archive_scope_all {
            format!("全部 {} 张", state.visible_count())
        } else {
            format!("选中 {} 张", state.selected_photo_ids().len())
        };
        let app_scope = app.clone();
        let scope_btn = Button::new("arc-scope")
            .outline()
            .label(scope_text)
            .on_click(move |_ev, _w, cx| {
                app_scope.update(cx, |s, cx| {
                    s.archive_scope_all = !s.archive_scope_all;
                    s.refresh_archive_plan();
                    cx.notify();
                });
            });

        // ④ 目标目录（原生对话框）
        let app_dest = app.clone();
        let dest_btn = Button::new("arc-choose-dest")
            .outline()
            .icon(IconName::FolderOpen)
            .label("选择归档目标目录")
            .on_click(move |_ev, _w, cx| {
                // 模态对话框在实体 update 之外打开，避免对话框消息泵重入借用。
                let picked = rfd::FileDialog::new()
                    .set_title("选择归档目标目录")
                    .pick_folder();
                if let Some(folder) = picked {
                    app_dest.update(cx, |s, cx| {
                        s.set_archive_dest(folder.to_path_buf());
                        cx.notify();
                    });
                }
            });

        // 完整目录树预览：展平成 (深度, 节点) 列表，缩进体现层级
        let mut flat: Vec<(usize, ArchiveTreeNode)> = Vec::new();
        if let Some(p) = &plan {
            for node in &p.tree {
                flatten_archive_node(node, 0, &mut flat);
            }
        }
        let tree_rows = flat
            .into_iter()
            .enumerate()
            .map(|(i, (depth, node))| {
                let name = node.name.clone();
                let count = node.total_count;
                let size = format!("{:.1} MB", node.total_bytes as f64 / 1024.0 / 1024.0);
                let sample = node.sample.join("、");
                div()
                    .id(("arc-row", i))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .pl(px(10. + depth as f32 * 18.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(col(BORDER_LIGHT))
                    .child(Icon::new(IconName::FolderOpen).size_3().text_color(col(GOLD)))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(col(TEXT_SECONDARY))
                            .child(format!("{} 张 · {}", count, size)),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .text_size(px(10.))
                            .text_color(col(TEXT_LIGHT))
                            .child(sample),
                    )
            })
            .collect::<Vec<_>>();

        let tree_body: AnyElement = if tree_rows.is_empty() {
            div()
                .id("arc-tree-empty")
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(Icon::new(IconName::FolderOpen).size(px(40.)).text_color(col(BORDER_LIGHT)))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(col(TEXT_LIGHT))
                        .child("当前范围没有照片，请先在相册库扫描或调整范围"),
                )
                .into_any_element()
        } else {
            div()
                .id("arc-tree-rows")
                .flex()
                .flex_col()
                .children(tree_rows)
                .into_any_element()
        };

        let tree_panel = div()
            .id("arc-tree-panel")
            .flex_1()
            .min_h_0()
            .bg(col(BG_WHITE))
            .rounded_lg()
            .p(px(16.))
            .shadow_sm()
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(col(TEXT_SECONDARY))
                    .mb(px(10.))
                    .child("🌳 完整目录树预览（与实际落盘一致）"),
            )
            .child(div().id("arc-tree-scroll").flex_1().min_h_0().overflow_y_scroll().child(tree_body));

        let stats_panel = div()
            .id("arc-stats")
            .w(px(250.))
            .flex_none()
            .bg(col(BG_WHITE))
            .rounded_lg()
            .p(px(16.))
            .shadow_sm()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("📊 本次归档"),
            )
            .child(archive_stat_row("待归档", format!("{} 张", total_files)))
            .child(archive_stat_row(
                "占用空间",
                format!("{:.1} MB", total_bytes as f64 / 1024.0 / 1024.0),
            ))
            .child(archive_stat_row("冲突改名", format!("{} 张", conflict_count)))
            .child(archive_stat_row("目录层级", rule_label.clone()))
            .child(archive_stat_row("执行方式", mode_label.clone()))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(TEXT_LIGHT))
                    .child(format!("目标：{}", dest_label)),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(col(TEXT_LIGHT))
                    .child("复制模式会保留源文件并把副本加入索引；撤销只删除本次副本。"),
            );

        let app_refresh = app.clone();
        let refresh_btn = Button::new("arc-refresh")
            .outline()
            .icon(IconName::Redo)
            .label("刷新预览")
            .on_click(move |_ev, _w, cx| {
                app_refresh.update(cx, |s, cx| {
                    s.refresh_archive_plan();
                    cx.notify();
                });
            });
        let app_undo = app.clone();
        let undo_btn = Button::new("arc-undo")
            .outline()
            .icon(IconName::Undo)
            .label("撤销上次归档")
            .disabled(state.last_archive_op.is_none())
            .on_click(move |_ev, _w, cx| {
                app_undo.update(cx, |s, cx| {
                    s.undo_archive();
                    cx.notify();
                });
            });
        let app_exec = app.clone();
        let exec_btn = Button::new("arc-exec")
            .primary()
            .icon(IconName::FolderOpen)
            .label(format!("执行归档（{} 张）", total_files))
            .disabled(total_files == 0)
            .on_click(move |_ev, _w, cx| {
                app_exec.update(cx, |s, cx| {
                    s.execute_archive();
                    cx.notify();
                });
            });

        let status = if state.ops_status.is_empty() {
            format!("规则：{} · 模式：{} · 目录树预览只读，确认后再执行", rule_label, mode_label)
        } else {
            state.ops_status.clone()
        };

        div()
            .id("page-archive")
            .flex()
            .flex_col()
            .child(page_title("🗂️ 智能归档", "按规则生成目录树 · 预览后执行 · 可一次撤销"))
            .child(
                div()
                    .id("arc-status")
                    .text_size(px(12.))
                    .text_color(col(TEXT_SECONDARY))
                    .my(px(10.))
                    .child(status),
            )
            .child(
                div()
                    .id("arc-config")
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .mb(px(14.))
                    .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child("① 归档规则"))
                    .child(div().flex().flex_row().gap(px(10.)).children(rule_cards))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(12.)).child("② 模式"))
                            .child(move_btn)
                            .child(copy_btn)
                            .child(div().text_size(px(12.)).ml(px(6.)).child("③ 范围"))
                            .child(scope_btn),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(12.)).child("④ 目标目录"))
                            .child(dest_btn)
                            .child(div().text_size(px(10.)).text_color(col(TEXT_LIGHT)).child(dest_label.clone())),
                    ),
            )
            .child(
                div()
                    .id("arc-main")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .gap(px(14.))
                    .child(tree_panel)
                    .child(stats_panel),
            )
            .child(
                div()
                    .id("arc-actions")
                    .flex()
                    .flex_row()
                    .gap(px(10.))
                    .mt(px(14.))
                    .justify_end()
                    .child(refresh_btn)
                    .child(undo_btn)
                    .child(exec_btn),
            )
    }

    // ===== 设置与隐私中心（M5） =====
    fn page_settings(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        let paths = state.paths.clone();
        let card = |title: &'static str, children: Vec<AnyElement>| -> AnyElement {
            div()
                .bg(col(BG_WHITE))
                .rounded_lg()
                .px(px(20.))
                .py(px(16.))
                .shadow_sm()
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(col(PRIMARY))
                        .mb(px(10.))
                        .child(title.to_owned()),
                )
                .children(children)
                .into_any_element()
        };

        // 错误态统一由 AppRoot 的全局横幅展示，避免只在部分页面可见。
        let banner: AnyElement = div().id("settings-banner-none").into_any_element();

        // 1) 本机数据路径（全部来自 Paths，非硬编码）
        let paths_for_btn = paths.clone();
        let open_dir_btn = Button::new("settings-open-dir")
            .outline()
            .icon(IconName::FolderOpen)
            .label("打开数据目录")
            .on_click(move |_ev, _w, _cx| {
                reveal_in_explorer(&paths_for_btn.data_dir);
            });
        let path_rows: Vec<(&'static str, String)> = vec![
            ("数据目录", paths.data_dir.display().to_string()),
            ("索引库", paths.db_path.display().to_string()),
            ("缩略图缓存", paths.cache_dir.display().to_string()),
            ("回收站", paths.trash_dir.display().to_string()),
            ("日志文件", paths.log_path.display().to_string()),
        ];
        let path_children: Vec<AnyElement> = path_rows
            .into_iter()
            .map(|(label, value)| info_block(label, value))
            .chain(std::iter::once(
                div().mt(px(6.)).child(open_dir_btn).into_any_element(),
            ))
            .collect();
        let path_card = card("💾 本机数据", path_children);

        // 2) 整理偏好：回收站保留期（持久化到 settings，回收站页清理按钮跟随）
        let retention = state.retention_days;
        let app_ret = app.clone();
        let retention_buttons = [7_i64, 30_i64, 90_i64]
            .into_iter()
            .enumerate()
            .map(|(i, days)| {
                let app = app_ret.clone();
                Button::new(("settings-retention", i))
                    .when(days == retention, |b| b.primary())
                    .when(days != retention, |b| b.outline())
                    .label(format!("{} 天", days))
                    .on_click(move |_ev, _w, cx| {
                        app.update(cx, |s, cx| {
                            let result = crate::core::db::open(&s.paths.db_path)
                                .map_err(anyhow::Error::from)
                                .and_then(|conn| ops_trash::set_retention_days(&conn, days));
                            match result {
                                Ok(()) => {
                                    s.retention_days = days;
                                    s.ops_status = format!("回收站保留期已设为 {} 天", days);
                                    s.last_error = None;
                                }
                                Err(e) => {
                                    s.ops_status = format!("保存设置失败：{e}");
                                    s.last_error = Some(format!("保存设置失败：{e}"));
                                }
                            }
                            cx.notify();
                        });
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let retention_children: Vec<AnyElement> = vec![
            div()
                .flex().flex_row()
                .items_center()
                .gap(px(10.))
                .children(retention_buttons)
                .into_any_element(),
            div()
                .mt(px(8.))
                .text_size(px(11.))
                .text_color(col(TEXT_LIGHT))
                .child(format!(
                    "当前保留 {} 天；回收站页「清理 N 天前」按钮与说明会跟随此设置，配置保存在本机索引库中。",
                    retention
                ))
                .into_any_element(),
        ];
        let retention_card = card("🗂 整理偏好", retention_children);

        // 3) 数据概况：全部来自真实索引与操作日志
        let overview_rows: Vec<(&'static str, String)> = vec![
            ("已索引照片", format!("{} 张", state.data.total_count)),
            ("占用空间", human_size(state.data.total_bytes)),
            ("扫描目录", format!("{} 个", state.data.root_count)),
            (
                "回收站",
                format!("{} 张 · {}", state.ops_data.trash_count, state.ops_data.size_label()),
            ),
            ("操作记录", format!("{} 条", state.ops_data.recent_ops.len())),
            ("已分析照片", format!("{} 张", state.analyze_data.analyzed_count)),
        ];
        let overview_children: Vec<AnyElement> = overview_rows
            .into_iter()
            .map(|(label, value)| info_row(label, value))
            .collect();
        let overview_card = card("📊 数据概况", overview_children);

        // 4) 隐私说明
        let privacy_lines = [
            "全部索引、缩略图、操作日志与回收站文件都保存在上方「数据目录」，不上传任何照片或元数据。",
            "不需要账号、不联网、不含遥测；断网状态下扫描、分析、整理、归档、撤销全部可用。",
            "删除一律先把文件移动到回收站，可恢复或一次撤销；只有「清空回收站 / 清理过期项」才会永久删除。",
            "归档默认目标为扫描根目录的同级目录，避免归档副本被下次扫描重复索引。",
        ];
        let privacy_children: Vec<AnyElement> = privacy_lines
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                div()
                    .id(("settings-privacy", i))
                    .flex().flex_row()
                    .gap(px(8.))
                    .py(px(5.))
                    .text_size(px(12.))
                    .text_color(col(TEXT_SECONDARY))
                    .child(Icon::new(IconName::CircleCheck).size_3().text_color(col(GOLD)))
                    .child(line.to_owned())
                    .into_any_element()
            })
            .collect();
        let privacy_card = card("🔒 隐私", privacy_children);

        // 5) 关于
        let about_rows: Vec<(&'static str, &'static str)> = vec![
            ("应用", "Akashic · 相册整理师 v0.1.0"),
            ("构建", "Rust edition 2024 · gpui 0.2.2 / gpui-component 0.5.1"),
            ("索引", "SQLite（rusqlite bundled，WAL）"),
            ("文档", "CODEBUDDY.md → .docs/WORKFLOW.md → .docs/INDEX.md / DEV_PLAN.md"),
        ];
        let about_children: Vec<AnyElement> = about_rows
            .into_iter()
            .map(|(label, value)| info_row(label, value.to_owned()))
            .collect();
        let about_card = card("ℹ 关于", about_children);

        let app_home = app.clone();
        let home_btn = Button::new("settings-home")
            .ghost()
            .icon(IconName::LayoutDashboard)
            .label("返回首页")
            .on_click(move |_ev, _w, cx| {
                app_home.update(cx, |s, cx| {
                    s.page = Page::Home;
                    cx.notify();
                });
            });

        div()
            .id("page-settings")
            .flex().flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex().flex_row()
                    .justify_between()
                    .items_center()
                    .child(page_title("⚙ 设置与隐私", "全部数据仅存本机 · 离线可用"))
                    .child(home_btn),
            )
            .child(banner)
            .child(path_card)
            .child(retention_card)
            .child(overview_card)
            .child(privacy_card)
            .child(about_card)
    }

    // ===== 回收站页（M3） =====
    fn page_trash(&self, state: &AppState) -> impl IntoElement + 'static {
        let app = &self.app;
        let ops = &state.ops_data;
        let editor_paths = state.paths.clone();

        let rows = ops
            .trash
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let app = app.clone();
                let editor_paths = editor_paths.clone();
                let id = item.id;
                let name = item.photo.file_name.clone();
                let size = item.photo.file_size_label();
                // 原图已移动到回收站目录，重定向到真实存在的文件，保证大图/按钮可用
                let mut thumb_photo = item.photo.clone();
                thumb_photo.path = item.trash_path.clone();
                let age_days = (crate::core::db::now_millis() - item.trashed_at)
                    .max(0)
                    / (24 * 60 * 60 * 1000);
                div()
                    .id(("trash-row", i))
                    .flex().flex_row().items_center().gap(px(12.))
                    .px(px(14.)).py(px(10.))
                    .bg(col(BG_WARM)).rounded_md()
                    .child(
                        div()
                            .id(("trash-thumb", i))
                            .w(px(44.)).h(px(44.))
                            .flex_none()
                            .rounded_md()
                            .overflow_hidden()
                            .cursor_pointer()
                            .child(photo_thumb(&thumb_photo))
                            .on_click(move |_ev, _window, cx| {
                                open_photo_editor(thumb_photo.clone(), editor_paths.clone(), cx)
                            }),
                    )
                    .child(div().w(px(300.)).text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(name))
                    .child(div().text_size(px(11.)).text_color(col(TEXT_SECONDARY)).child(size))
                    .child(div().text_size(px(11.)).text_color(col(TEXT_LIGHT)).child(format!("{} 天前", age_days)))
                    .child(
                        Button::new(("trash-restore", i)).outline().icon(IconName::Undo).label("恢复")
                            .on_click(move |_ev, _w, cx| {
                                let id = id;
                                app.update(cx, move |s, cx| {
                                    let paths = s.paths.clone();
                                    let result = crate::core::db::open(&paths.db_path)
                                        .map_err(anyhow::Error::from)
                                        .and_then(|conn| ops_trash::restore_items(&paths, &conn, &[id]));
                                    match result {
                                        Ok(_) => {
                                            s.ops_status = "已恢复到原路径".to_string();
                                            s.reload_all();
                                        }
                                        Err(e) => s.ops_status = format!("恢复失败：{e}"),
                                    }
                                    cx.notify();
                                });
                            }),
                    )
            })
            .collect::<Vec<_>>();

        let app_empty = app.clone();
        let empty_btn = Button::new("trash-empty").outline().icon(IconName::Delete)
            .label("清空回收站").text_color(col(DANGER))
            .on_click(move |_ev, _w, cx| {
                app_empty.update(cx, |s, cx| {
                    let paths = s.paths.clone();
                    let result = crate::core::db::open(&paths.db_path)
                        .map_err(anyhow::Error::from)
                        .and_then(|conn| ops_trash::empty_trash(&paths, &conn));
                    match result {
                        Ok(n) => {
                            s.ops_status = format!("已永久清空回收站 {} 张", n);
                            // 文件已永久删除，原来的「撤销上次删除」不再有效。
                            s.last_trash_op = None;
                        }
                        Err(e) => s.ops_status = format!("清空失败：{e}"),
                    }
                    s.reload_all();
                    cx.notify();
                });
            });
        let app_purge = app.clone();
        let purge_btn = Button::new("trash-purge").ghost().icon(IconName::Redo).label(format!("清理 {} 天前", state.retention_days))
            .on_click(move |_ev, _w, cx| {
                app_purge.update(cx, |s, cx| {
                    let paths = s.paths.clone();
                    let days = s.retention_days;
                    let result = crate::core::db::open(&paths.db_path)
                        .map_err(anyhow::Error::from)
                        .and_then(|conn| ops_trash::purge_expired(&conn, days));
                    match result {
                        Ok(n) => {
                            s.ops_status = format!("已清理 {} 天前项目 {} 张", days, n);
                            s.last_trash_op = None;
                        }
                        Err(e) => s.ops_status = format!("清理失败：{e}"),
                    }
                    s.reload_all();
                    cx.notify();
                });
            });
        let app_undo = app.clone();
        let undo_btn = Button::new("trash-undo").ghost().icon(IconName::Undo).label("撤销上次删除")
            .disabled(state.last_trash_op.is_none())
            .on_click(move |_ev, _w, cx| {
                app_undo.update(cx, |s, cx| {
                    s.undo_trash();
                    cx.notify();
                });
            });

        let status = if state.ops_status.is_empty() {
            format!("回收站中的文件保留 {} 天，之后可手动清理；恢复会放回原路径。当前 {} 张 · {}", state.retention_days, ops.trash_count, ops.size_label())
        } else {
            state.ops_status.clone()
        };

        div()
            .id("page-trash")
            .flex().flex_col()
            .child(page_title("🗑️ 回收站", "删除的照片保存在这里 · 不会永久丢失"))
            .child(div().id("trash-status").text_size(px(12.)).text_color(col(TEXT_SECONDARY))
                .my(px(10.)).child(status))
            .child(
                div().id("trash-list").flex_1().bg(col(BG_WHITE)).rounded_lg()
                    .p(px(18.)).shadow_sm().flex().flex_col().gap(px(8.))
                    .child(if rows.is_empty() {
                        div().id("trash-empty").flex_1().flex().flex_col().items_center()
                            .justify_center().gap(px(8.))
                            .child(Icon::new(IconName::Delete).size(px(40.)).text_color(col(BORDER_LIGHT)))
                            .child(div().text_size(px(13.)).text_color(col(TEXT_LIGHT)).child("回收站是空的"))
                            .into_any_element()
                    } else {
                        div().id("trash-rows").flex().flex_col().gap(px(6.)).children(rows).into_any_element()
                    })
            )
            .child(div().id("trash-actions").flex().flex_row().gap(px(10.)).mt(px(14.)).justify_end()
                .child(undo_btn).child(purge_btn).child(empty_btn))
    }

}

/// 把归档目录树展平成 (深度, 节点) 列表，供 UI 按层级缩进渲染。
fn flatten_archive_node(
    node: &ArchiveTreeNode,
    depth: usize,
    out: &mut Vec<(usize, ArchiveTreeNode)>,
) {
    out.push((depth, node.clone()));
    for child in &node.children {
        flatten_archive_node(child, depth + 1, out);
    }
}

/// 归档统计面板的一行。
fn archive_stat_row(label: &'static str, value: String) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .text_size(px(12.))
        .child(div().text_color(col(TEXT_SECONDARY)).child(label))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
        .into_any_element()
}

/// 根据路径稳定生成色相，保证同一照片每次显示相同占位色。
fn stable_hue(s: &str) -> f32 {
    let mut h: u32 = 0;
    for b in s.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    (h % 360) as f32
}

/// 首页副标题：错误态 / 加载态 / 正常态均来自真实状态。
fn subtitle_for_home(state: &AppState) -> String {
    if let Some(err) = state.load_error.as_ref().or(state.last_error.as_ref()) {
        return format!("索引或后台任务异常 · {err}");
    }
    if state.is_scanning {
        return format!("正在扫描 · {}", state.scan_status);
    }
    if state.is_analyzing {
        return format!("正在分析 · {}", state.analyze_status);
    }
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

// ===== 照片图片渲染 =====

/// `img()` 能解码的扩展名（与 `Img::extensions()` 对齐，忽略大小写）。
/// 不支持的格式（HEIC/RAW 等）不喂给 gpui，避免解码失败白屏。
fn is_decodable_image(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_ascii_lowercase();
            Img::extensions().iter().any(|ext| *ext == lower.as_str())
        })
        .unwrap_or(false)
}

/// 网格/列表缩略图源：优先缩略图缓存；缓存缺失时回退到原图（仅可解码格式）。
fn thumbnail_source(photo: &Photo) -> Option<PathBuf> {
    // 原图已被外部移动/删除时，不再拿旧缓存冒充有效照片，避免显示「幽灵缩略图」。
    if !photo.path.exists() {
        return None;
    }
    if let Some(t) = &photo.thumbnail_path {
        if t.exists() {
            return Some(t.clone());
        }
    }
    if is_decodable_image(&photo.path) {
        return Some(photo.path.clone());
    }
    None
}

/// 详情大图源：优先原图（真正「查看原图」），原图不可解码时回退缩略图缓存。
fn preview_source(photo: &Photo) -> Option<PathBuf> {
    if photo.path.exists() && is_decodable_image(&photo.path) {
        return Some(photo.path.clone());
    }
    if let Some(t) = &photo.thumbnail_path {
        if t.exists() {
            return Some(t.clone());
        }
    }
    None
}

/// 无图 / 图片加载中 / 加载失败时的色块占位，保持 M0 视觉。
fn photo_placeholder(hue: f32, location: &str, name: &str) -> AnyElement {
    div()
        .size_full()
        .flex().flex_col()
        .items_center().justify_center()
        .gap(px(6.))
        .bg(photo_block_bg(hue))
        .child(
            div()
                .size(px(30.))
                .rounded_full()
                .bg(photo_icon_bg(hue))
                .text_color(rgba(255, 255, 255, 0.95))
                .items_center().justify_center()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(location.chars().next().unwrap_or('图').to_string()),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgba(255, 255, 255, 0.9))
                .child(name.to_string()),
        )
        .into_any_element()
}

/// 照片缩略图元素：优先真实图片（异步加载、Cover 铺满），任何失败都降级到色块。
/// 元素自身 `size_full()`，调用方需提供有确定尺寸的父容器。
fn photo_thumb(photo: &Photo) -> AnyElement {
    let hue = stable_hue(&photo.path.to_string_lossy());
    let location = photo.location.clone().unwrap_or_default();
    let name = photo.file_name.clone();
    match thumbnail_source(photo) {
        Some(path) => {
            let fb_loc = location.clone();
            let fb_name = name.clone();
            let ld_loc = location.clone();
            let ld_name = name.clone();
            img(path)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .with_fallback(move || photo_placeholder(hue, &fb_loc, &fb_name))
                .with_loading(move || photo_placeholder(hue, &ld_loc, &ld_name))
                .into_any_element()
        }
        None => photo_placeholder(hue, &location, &name),
    }
}

/// 毫秒时间戳 → 可读日期（UTC）。
/// 人类可读的字节数（首页统计 / 设置页数据概况共用）。
fn human_size(bytes: i64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// 按字符数截断长文本，超出时追加省略号。
fn truncate(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

fn format_millis(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// 用系统默认程序打开原图。
#[cfg(target_os = "windows")]
fn open_with_default(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}
#[cfg(target_os = "macos")]
fn open_with_default(path: &std::path::Path) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn open_with_default(path: &std::path::Path) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

/// 在文件管理器中定位原图。
#[cfg(target_os = "windows")]
fn reveal_in_explorer(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
}
#[cfg(target_os = "macos")]
fn reveal_in_explorer(path: &std::path::Path) {
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
}
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn reveal_in_explorer(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}

// ===== 照片详情对话框 =====

/// 打开照片详情：应用内显示原图大图 + 完整图像信息。
/// 接受一份 owned Photo，相册库 / 整理结果 / 回收站均可调用。
pub(crate) fn open_photo_dialog(photo: Photo, window: &mut Window, cx: &mut App) {
    let name = photo.file_name.clone();
    let path_str = photo.path.to_string_lossy().to_string();
    let parent_str = photo
        .path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let date = photo.date_label();
    let modified = format_millis(photo.modified_at);
    let size = photo.size_label();
    let file_size = photo.file_size_label();
    let device = photo.device.clone().unwrap_or_else(|| "未知设备".to_string());
    let location = photo.location.clone().unwrap_or_else(|| "未知地点".to_string());
    let tags: Vec<String> = photo.tag_list().into_iter().map(|s| s.to_string()).collect();
    let hash_short: String = photo.content_hash.chars().take(16).collect();
    let blur = photo
        .blur_score
        .map(|v| format!("{:.1}", v))
        .unwrap_or_else(|| "未分析".to_string());
    let screenshot = (if photo.is_screenshot { "是" } else { "否" }).to_string();
    let group = photo
        .group_id
        .map(|g| format!("#{}", g))
        .unwrap_or_else(|| "无".to_string());
    let phash = photo
        .phash
        .map(|h| format!("{:016X}", h))
        .unwrap_or_else(|| "—".to_string());
    let gps = match (photo.gps_lat_wgs84, photo.gps_lon_wgs84) {
        (Some(lat), Some(lon)) => format!("{:.5}, {:.5}", lat, lon),
        _ => "未采集".to_string(),
    };
    let thumb_state = photo
        .thumbnail_path
        .as_ref()
        .filter(|p| p.exists())
        .map(|_| "已缓存".to_string())
        .unwrap_or_else(|| "无缓存".to_string());
    let hue = stable_hue(&path_str);

    let big_path = preview_source(&photo);
    let is_original = big_path.as_ref().map(|p| *p == photo.path).unwrap_or(false);
    let preview_hint = match &big_path {
        Some(_) if is_original => "原图预览 · 完整显示".to_string(),
        Some(_) => "缩略图预览（原图不可解码或已移动）".to_string(),
        None => "无法预览（缺少可解码的图片文件）".to_string(),
    };

    let open_path = photo.path.clone();
    let reveal_path = photo.path.clone();
    // 文件被移动/删除后按钮不能「点了没反应」：直接禁用并改文案。
    let original_exists = photo.path.exists();

    // 大图元素工厂：dialog 每次重绘都会调用，因此必须是可重复调用的 Fn。
    let make_big = {
        let big_loc = location.clone();
        let big_name = name.clone();
        move || -> AnyElement {
            match &big_path {
                Some(path) => {
                    let l1 = big_loc.clone();
                    let n1 = big_name.clone();
                    let l2 = big_loc.clone();
                    let n2 = big_name.clone();
                    img(path.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .with_fallback(move || photo_placeholder(hue, &l1, &n1))
                        .with_loading(move || photo_placeholder(hue, &l2, &n2))
                        .into_any_element()
                }
                None => photo_placeholder(hue, &big_loc, &big_name),
            }
        }
    };

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

        let p_open = open_path.clone();
        let open_btn = dialog_btn(
            IconName::Frame,
            if original_exists { "打开原图" } else { "原图已移动" },
            PRIMARY,
        )
        .disabled(!original_exists)
        .on_click(move |_ev, _w, _cx| open_with_default(&p_open));
        let p_reveal = reveal_path.clone();
        let reveal_btn = dialog_btn(
            IconName::FolderOpen,
            if original_exists { "所在文件夹" } else { "原路径不可用" },
            TEXT_SECONDARY,
        )
        .disabled(!original_exists)
        .on_click(move |_ev, _w, _cx| reveal_in_explorer(&p_reveal));

        dialog
            .title(format!("照片详情 · {}", name))
            .w(px(940.))
            .h(px(600.))
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
                            .min_w_0()
                            .min_h_0()
                            .flex().flex_col()
                            .bg(col(BG_WARM))
                            .rounded_md()
                            .overflow_hidden()
                            .child(
                                div()
                                    .id("dialog-image-area")
                                    .flex_1()
                                    .min_h_0()
                                    .p(px(10.))
                                    .child(make_big()),
                            )
                            .child(
                                div()
                                    .id("dialog-preview-hint")
                                    .text_size(px(11.))
                                    .text_color(col(TEXT_SECONDARY))
                                    .px(px(12.))
                                    .py(px(6.))
                                    .child(preview_hint.clone()),
                            ),
                    )
                    .child(
                        div()
                            .id("dialog-info")
                            .w(px(320.))
                            .min_w(px(320.))
                            .px(px(20.))
                            .py(px(12.))
                            .border_l_1()
                            .border_color(col(BORDER_LIGHT))
                            .overflow_y_scroll()
                            .flex().flex_col()
                            .child(section_title("基本信息"))
                            .child(info_row("文件名", name.clone()))
                            .child(info_row("拍摄日期", date.clone()))
                            .child(info_row("修改时间", modified.clone()))
                            .child(info_row("尺寸", size.clone()))
                            .child(info_row("文件大小", file_size.clone()))
                            .child(info_row("设备", device.clone()))
                            .child(info_row("地点", location.clone()))
                            .child(info_block("所在目录", parent_str.clone()))
                            .child(info_tag_section("标签", build_tags()))
                            .child(section_title("分析结果"))
                            .child(info_row("模糊评分", blur.clone()))
                            .child(info_row("截图", screenshot.clone()))
                            .child(info_row("相似组", group.clone()))
                            .child(info_row("感知哈希", phash.clone()))
                            .child(info_row("GPS", gps.clone()))
                            .child(section_title("索引"))
                            .child(info_row("内容哈希", hash_short.clone()))
                            .child(info_row("缩略图", thumb_state.clone()))
                            .child(
                                div()
                                    .id("dialog-actions")
                                    .mt_4()
                                    .flex().flex_row()
                                    .gap(px(6.))
                                    .children(vec![open_btn, reveal_btn]),
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
        .gap(px(12.))
        .py(px(6.))
        .child(div().flex_none().text_color(col(TEXT_SECONDARY)).child(label))
        .child(div().text_color(col(PRIMARY)).font_weight(FontWeight::MEDIUM).child(value.into()))
        .into_any_element()
}

/// 竖排信息块，用于较长的值（如完整目录路径）。
fn info_block(label: &'static str, value: impl Into<SharedString>) -> AnyElement {
    div()
        .flex().flex_col()
        .gap(px(2.))
        .py(px(6.))
        .child(div().text_size(px(11.)).text_color(col(TEXT_SECONDARY)).child(label))
        .child(div().text_size(px(11.)).text_color(col(PRIMARY)).child(value.into()))
        .into_any_element()
}

fn section_title(text: &'static str) -> AnyElement {
    div()
        .mt_3()
        .mb(px(4.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(col(PRIMARY_LIGHTER))
        .child(text.to_owned())
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
