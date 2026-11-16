//! 图标资源：编译期嵌入 assets/icons 目录的 Lucide SVG。

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[derive(Clone)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let file: Option<&[u8]> = match path {
            "icons/layout-dashboard.svg" => Some(include_bytes!("../../assets/icons/layout-dashboard.svg")),
            "icons/settings-2.svg" => Some(include_bytes!("../../assets/icons/settings-2.svg")),
            "icons/gallery-vertical-end.svg" => Some(include_bytes!("../../assets/icons/gallery-vertical-end.svg")),
            "icons/case-sensitive.svg" => Some(include_bytes!("../../assets/icons/case-sensitive.svg")),
            "icons/delete.svg" => Some(include_bytes!("../../assets/icons/delete.svg")),
            "icons/settings.svg" => Some(include_bytes!("../../assets/icons/settings.svg")),
            "icons/copy.svg" => Some(include_bytes!("../../assets/icons/copy.svg")),
            "icons/eye-off.svg" => Some(include_bytes!("../../assets/icons/eye-off.svg")),
            "icons/frame.svg" => Some(include_bytes!("../../assets/icons/frame.svg")),
            "icons/file.svg" => Some(include_bytes!("../../assets/icons/file.svg")),
            "icons/folder-open.svg" => Some(include_bytes!("../../assets/icons/folder-open.svg")),
            "icons/book-open.svg" => Some(include_bytes!("../../assets/icons/book-open.svg")),
            "icons/star.svg" => Some(include_bytes!("../../assets/icons/star.svg")),
            "icons/search.svg" => Some(include_bytes!("../../assets/icons/search.svg")),
            "icons/close.svg" => Some(include_bytes!("../../assets/icons/close.svg")),
            "icons/chevron-right.svg" => Some(include_bytes!("../../assets/icons/chevron-right.svg")),
            "icons/check.svg" => Some(include_bytes!("../../assets/icons/check.svg")),
            "icons/info.svg" => Some(include_bytes!("../../assets/icons/info.svg")),
            "icons/circle-user.svg" => Some(include_bytes!("../../assets/icons/circle-user.svg")),
            "icons/circle-check.svg" => Some(include_bytes!("../../assets/icons/circle-check.svg")),
            "icons/redo.svg" => Some(include_bytes!("../../assets/icons/redo.svg")),
            "icons/plus.svg" => Some(include_bytes!("../../assets/icons/plus.svg")),
            _ => None,
        };
        Ok(file.map(|bytes| Cow::Owned(bytes.to_vec())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
