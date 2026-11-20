//! 核心层：不依赖 gpui，可独立单元测试。
//!
//! 依赖方向约束：`main.rs → ui → core → 外部设施`。core 不得引用 gpui。

pub mod analyze;
pub mod config;
pub mod db;
pub mod edit;
pub mod model;
pub mod ops;
pub mod scanner;
pub mod thumbnail;
