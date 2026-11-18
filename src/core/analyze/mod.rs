//! 分析子层：模糊评分、相似归组、截图/大文件筛选。
//!
//! 不依赖 gpui，可独立单元测试。所有分析以缩略图（或原图）为输入，
//! 结果通过 `analyze_library` 汇总后由 `db` 落库。

pub mod blur;
pub mod classify;
pub mod pipeline;
pub mod similarity;

pub use pipeline::{AnalyzeProgress, AnalyzeReport, PhotoAnalysis, analyze_library, analyze_photo};
