//! UI 层：只依赖 core 与 gpui，不直接访问文件系统（M1 起由 core 提供数据）。

pub mod app;
pub mod assets;
pub mod library;
