use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/arrow-left.svg",
        include_bytes!("../assets/icons/arrow-left.svg"),
    ),
    (
        "icons/arrow-right.svg",
        include_bytes!("../assets/icons/arrow-right.svg"),
    ),
    (
        "icons/arrow-up.svg",
        include_bytes!("../assets/icons/arrow-up.svg"),
    ),
    (
        "icons/calendar.svg",
        include_bytes!("../assets/icons/calendar.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-left.svg",
        include_bytes!("../assets/icons/chevron-left.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/chevrons-up-down.svg",
        include_bytes!("../assets/icons/chevrons-up-down.svg"),
    ),
    (
        "icons/circle-check.svg",
        include_bytes!("../assets/icons/circle-check.svg"),
    ),
    (
        "icons/circle-x.svg",
        include_bytes!("../assets/icons/circle-x.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/close.svg"),
    ),
    (
        "icons/delete.svg",
        include_bytes!("../assets/icons/delete.svg"),
    ),
    (
        "icons/folder-closed.svg",
        include_bytes!("../assets/icons/folder-closed.svg"),
    ),
    (
        "icons/frame.svg",
        include_bytes!("../assets/icons/frame.svg"),
    ),
    (
        "icons/gallery-vertical-end.svg",
        include_bytes!("../assets/icons/gallery-vertical-end.svg"),
    ),
    (
        "icons/heart-filled.svg",
        include_bytes!("../assets/icons/heart-filled.svg"),
    ),
    (
        "icons/heart-outline.svg",
        include_bytes!("../assets/icons/heart-outline.svg"),
    ),
    (
        "icons/heart.svg",
        include_bytes!("../assets/icons/heart.svg"),
    ),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    (
        "icons/panel-left-close.svg",
        include_bytes!("../assets/icons/panel-left-close.svg"),
    ),
    (
        "icons/panel-left-open.svg",
        include_bytes!("../assets/icons/panel-left-open.svg"),
    ),
    (
        "icons/panel-right-close.svg",
        include_bytes!("../assets/icons/panel-right-close.svg"),
    ),
    (
        "icons/panel-right-open.svg",
        include_bytes!("../assets/icons/panel-right-open.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/triangle-alert.svg",
        include_bytes!("../assets/icons/triangle-alert.svg"),
    ),
    (
        "icons/window-close.svg",
        include_bytes!("../assets/icons/window-close.svg"),
    ),
    (
        "icons/window-maximize.svg",
        include_bytes!("../assets/icons/window-maximize.svg"),
    ),
    (
        "icons/window-minimize.svg",
        include_bytes!("../assets/icons/window-minimize.svg"),
    ),
    (
        "icons/window-restore.svg",
        include_bytes!("../assets/icons/window-restore.svg"),
    ),
];

/// 应用按需 SVG 资源包：只嵌入当前 UI 与 gpui-component 内部实际会用到的图标。
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find_map(|(icon_path, bytes)| (*icon_path == path).then_some(Cow::Borrowed(*bytes))))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(icon_path, _)| icon_path.starts_with(path))
            .map(|(icon_path, _)| (*icon_path).into())
            .collect())
    }
}
