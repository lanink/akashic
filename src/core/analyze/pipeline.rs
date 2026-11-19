//! 分析编排：遍历索引中的照片，逐个评分并把结果写回 SQLite。
//!
//! 不依赖 gpui，可在后台线程运行；进度通过 `AnalyzeProgress` 回传 UI。

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::{blur, classify, similarity};
use crate::core::db;
use crate::core::model::Photo;

/// 分析进度事件（后台线程 → UI）。
#[derive(Clone, Debug)]
pub enum AnalyzeProgress {
    Started { total: usize },
    Analyzing { processed: usize, total: usize },
    Finished { report: AnalyzeReport },
    Failed { message: String },
}

/// 一次完整分析的统计结果。
#[derive(Clone, Debug, Default)]
pub struct AnalyzeReport {
    pub photos_analyzed: usize,
    pub photos_failed: usize,
    pub blurry_count: usize,
    pub screenshot_count: usize,
    pub similar_groups: usize,
    pub duration_ms: u128,
}

/// 单张照片的分析产出（写库前的内存结构）。
#[derive(Clone, Debug)]
pub struct PhotoAnalysis {
    pub photo_id: i64,
    pub blur_score: f64,
    pub is_screenshot: bool,
    pub phash: Option<u64>,
}

/// 分析输入源：优先缩略图（更小更快），无缩略图则回退原图。
fn analysis_source(photo: &Photo) -> PathBuf {
    if let Some(t) = &photo.thumbnail_path {
        if t.exists() {
            return t.clone();
        }
    }
    photo.path.clone()
}

/// 进度节流：每处理 N 张上报一次，避免 5000 张产生 5000 条消息把 Finished 挤到队尾。
const PROGRESS_EVERY: usize = 32;

/// 对单张照片执行全部分析（模糊评分 + 截图识别 + pHash）。
/// 解码失败返回 `None`：绝不能写成 `blur_score = 0.0`，否则坏图会被当成
/// 「已分析」且永远排在模糊列表最前面，还会污染 `count_analyzed` / `is_ready()`。
pub fn analyze_photo(photo: &Photo) -> Option<PhotoAnalysis> {
    let source = analysis_source(photo);
    let blur_score = blur::blur_score(&source).ok()?;
    let is_screenshot = classify::is_screenshot(photo);
    let phash = similarity::phash_from_path(&source).ok();
    Some(PhotoAnalysis {
        photo_id: photo.id,
        blur_score,
        is_screenshot,
        phash,
    })
}

/// 分析整个索引库，写回分析结果并生成相似分组。
///
/// 相似组的 dssim 精排是可选增强：当两张候选图的缩略图都存在时做二次确认，
/// 失败则回退到纯 pHash 分组，保证不会因为个别坏图中断整体分析。
pub fn analyze_library(
    conn: &Connection,
    progress: Option<&Sender<AnalyzeProgress>>,
) -> Result<AnalyzeReport> {
    let started = std::time::Instant::now();
    let photos = db::list_photos(conn).context("读取待分析照片失败")?;
    let total = photos.len();

    if let Some(tx) = progress {
        let _ = tx.send(AnalyzeProgress::Started { total });
    }

    let mut report = AnalyzeReport::default();
    let mut analyses: Vec<PhotoAnalysis> = Vec::with_capacity(total);
    let mut processed = 0usize;

    for photo in &photos {
        match analyze_photo(photo) {
            None => report.photos_failed += 1, // 解码/评分失败，保留原有 NULL 便于下次重试
            Some(a) => match db::update_analysis(
                conn,
                a.photo_id,
                a.blur_score,
                a.is_screenshot,
                a.phash,
            ) {
                Ok(()) => {
                    report.photos_analyzed += 1;
                    if blur::is_blurry(a.blur_score) {
                        report.blurry_count += 1;
                    }
                    if a.is_screenshot {
                        report.screenshot_count += 1;
                    }
                    analyses.push(a);
                }
                Err(_) => report.photos_failed += 1,
            },
        }

        processed += 1;
        if let Some(tx) = progress {
            if processed == 1 || processed % PROGRESS_EVERY == 0 {
                let _ = tx.send(AnalyzeProgress::Analyzing { processed, total });
            }
        }
    }

    // 相似归组：先 pHash 分组，再对候选组用 dssim 精排（尽力而为）
    let hash_items: Vec<(i64, u64)> = analyses
        .iter()
        .filter_map(|a| a.phash.map(|h| (a.photo_id, h)))
        .collect();
    let candidate_groups = similarity::group_by_phash(&hash_items, similarity::HAMMING_THRESHOLD);
    let path_by_id: std::collections::HashMap<i64, PathBuf> =
        photos.iter().map(|p| (p.id, analysis_source(p))).collect();

    let mut final_groups: Vec<Vec<i64>> = Vec::new();
    for group in candidate_groups {
        let refined = refine_group(&group, &path_by_id);
        final_groups.extend(refined);
    }
    report.similar_groups = final_groups.len();
    // created_at 是时间戳，不是本次分析耗时。
    db::save_similar_groups(conn, &final_groups, db::now_millis())?;

    report.duration_ms = started.elapsed().as_millis();

    if let Some(tx) = progress {
        let _ = tx.send(AnalyzeProgress::Finished {
            report: report.clone(),
        });
    }
    Ok(report)
}

/// 对一个 pHash 候选组做 dssim 精排：以第一张为基准，剔除结构差异过大的成员，
/// 拆出一个“核心组”。剩余成员若有 2 张以上则递归再成组。
fn refine_group(group: &[i64], paths: &std::collections::HashMap<i64, PathBuf>) -> Vec<Vec<i64>> {
    if group.len() < 2 {
        return Vec::new();
    }
    let anchor = match paths.get(&group[0]).map(|p| p.as_path()) {
        Some(p) if p.exists() => p,
        _ => return vec![group.to_vec()], // 无缩略图时回退到 pHash 结果
    };

    let mut close = vec![group[0]];
    let mut far = Vec::new();
    for &id in &group[1..] {
        let similar = paths
            .get(&id)
            .map(|p| p.as_path())
            .filter(|p| p.exists())
            .and_then(|p| similarity::dssim_distance(anchor, p).ok())
            .map(|d| d <= similarity::DSSIM_THRESHOLD)
            .unwrap_or(false); // 精排失败不并入核心组，避免坏图被无条件判为相似
        if similar {
            close.push(id);
        } else {
            far.push(id);
        }
    }

    let mut result = Vec::new();
    if close.len() >= 2 {
        result.push(close);
    }
    if far.len() >= 2 {
        result.extend(refine_group(&far, paths));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_source_prefers_existing_thumbnail() {
        // 构造一个存在的缩略图路径
        let dir = std::env::temp_dir();
        let thumb = dir.join("akashic_test_thumb.jpg");
        std::fs::write(&thumb, b"x").ok();
        let photo = Photo {
            id: 1,
            path: PathBuf::from("/no/such/original.jpg"),
            file_name: "x.jpg".into(),
            size_bytes: 1,
            date_taken: None,
            modified_at: 0,
            width: 0,
            height: 0,
            device: None,
            location: None,
            tags: String::new(),
            content_hash: String::new(),
            thumbnail_path: Some(thumb.clone()),
            gps_lon_wgs84: None,
            gps_lat_wgs84: None,
            blur_score: None,
            is_screenshot: false,
            phash: None,
            group_id: None,
        };
        assert_eq!(analysis_source(&photo), thumb);
        let _ = std::fs::remove_file(&thumb);
    }

    #[test]
    fn refine_group_falls_back_without_thumbnails() {
        let paths = std::collections::HashMap::new();
        let group = vec![1i64, 2, 3];
        assert_eq!(refine_group(&group, &paths), vec![group]);
    }
}
