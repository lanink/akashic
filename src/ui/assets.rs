//! 图标资源：编译期嵌入 assets/icons 目录的 Lucide SVG。
//!
//! 这里登记 assets/icons/ 下的全部 SVG：除应用页面直接使用的图标外，
//! gpui-component 的内部组件（Dialog 关闭按钮、下拉箭头、分页等）也会按
//! `IconName::path()` 请求 `icons/<name>.svg`，缺登记会导致图标空白。
//! 新增图标文件时需同步在此增加分支。

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[derive(Clone)]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let file: Option<&[u8]> = match path {
            "icons/arrow-left.svg" => Some(include_bytes!("../../assets/icons/arrow-left.svg")),
            "icons/arrow-right.svg" => Some(include_bytes!("../../assets/icons/arrow-right.svg")),
            "icons/book-open.svg" => Some(include_bytes!("../../assets/icons/book-open.svg")),
            "icons/bot.svg" => Some(include_bytes!("../../assets/icons/bot.svg")),
            "icons/calendar.svg" => Some(include_bytes!("../../assets/icons/calendar.svg")),
            "icons/case-sensitive.svg" => Some(include_bytes!("../../assets/icons/case-sensitive.svg")),
            "icons/chart-pie.svg" => Some(include_bytes!("../../assets/icons/chart-pie.svg")),
            "icons/check.svg" => Some(include_bytes!("../../assets/icons/check.svg")),
            "icons/chevron-down.svg" => Some(include_bytes!("../../assets/icons/chevron-down.svg")),
            "icons/chevron-left.svg" => Some(include_bytes!("../../assets/icons/chevron-left.svg")),
            "icons/chevron-right.svg" => Some(include_bytes!("../../assets/icons/chevron-right.svg")),
            "icons/chevron-up.svg" => Some(include_bytes!("../../assets/icons/chevron-up.svg")),
            "icons/chevrons-up-down.svg" => Some(include_bytes!("../../assets/icons/chevrons-up-down.svg")),
            "icons/circle-check.svg" => Some(include_bytes!("../../assets/icons/circle-check.svg")),
            "icons/circle-user.svg" => Some(include_bytes!("../../assets/icons/circle-user.svg")),
            "icons/clock.svg" => Some(include_bytes!("../../assets/icons/clock.svg")),
            "icons/close.svg" => Some(include_bytes!("../../assets/icons/close.svg")),
            "icons/copy.svg" => Some(include_bytes!("../../assets/icons/copy.svg")),
            "icons/delete.svg" => Some(include_bytes!("../../assets/icons/delete.svg")),
            "icons/ellipsis.svg" => Some(include_bytes!("../../assets/icons/ellipsis.svg")),
            "icons/eye-off.svg" => Some(include_bytes!("../../assets/icons/eye-off.svg")),
            "icons/eye.svg" => Some(include_bytes!("../../assets/icons/eye.svg")),
            "icons/file.svg" => Some(include_bytes!("../../assets/icons/file.svg")),
            "icons/folder-closed.svg" => Some(include_bytes!("../../assets/icons/folder-closed.svg")),
            "icons/folder-open.svg" => Some(include_bytes!("../../assets/icons/folder-open.svg")),
            "icons/folder.svg" => Some(include_bytes!("../../assets/icons/folder.svg")),
            "icons/frame.svg" => Some(include_bytes!("../../assets/icons/frame.svg")),
            "icons/gallery-vertical-end.svg" => Some(include_bytes!("../../assets/icons/gallery-vertical-end.svg")),
            "icons/heart.svg" => Some(include_bytes!("../../assets/icons/heart.svg")),
            "icons/info.svg" => Some(include_bytes!("../../assets/icons/info.svg")),
            "icons/layout-dashboard.svg" => Some(include_bytes!("../../assets/icons/layout-dashboard.svg")),
            "icons/load-circle.svg" => Some(include_bytes!("../../assets/icons/load-circle.svg")),
            "icons/menu.svg" => Some(include_bytes!("../../assets/icons/menu.svg")),
            "icons/moon.svg" => Some(include_bytes!("../../assets/icons/moon.svg")),
            "icons/palette.svg" => Some(include_bytes!("../../assets/icons/palette.svg")),
            "icons/panel-right.svg" => Some(include_bytes!("../../assets/icons/panel-right.svg")),
            "icons/plus.svg" => Some(include_bytes!("../../assets/icons/plus.svg")),
            "icons/redo.svg" => Some(include_bytes!("../../assets/icons/redo.svg")),
            "icons/search.svg" => Some(include_bytes!("../../assets/icons/search.svg")),
            "icons/settings-2.svg" => Some(include_bytes!("../../assets/icons/settings-2.svg")),
            "icons/settings.svg" => Some(include_bytes!("../../assets/icons/settings.svg")),
            "icons/star-off.svg" => Some(include_bytes!("../../assets/icons/star-off.svg")),
            "icons/star.svg" => Some(include_bytes!("../../assets/icons/star.svg")),
            "icons/sun.svg" => Some(include_bytes!("../../assets/icons/sun.svg")),
            "icons/triangle-alert.svg" => Some(include_bytes!("../../assets/icons/triangle-alert.svg")),
            "icons/undo.svg" => Some(include_bytes!("../../assets/icons/undo.svg")),
            "icons/user.svg" => Some(include_bytes!("../../assets/icons/user.svg")),
            _ => None,
        };
        Ok(file.map(|bytes| Cow::Owned(bytes.to_vec())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}
