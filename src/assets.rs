use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

const HEART_OUTLINE: &[u8] = include_bytes!("../assets/icons/heart-outline.svg");
const HEART_FILLED: &[u8] = include_bytes!("../assets/icons/heart-filled.svg");

/// 应用自定义资源包：先提供本项目自己的 SVG，再回退到 gpui-component 内置图标资源。
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "icons/heart-outline.svg" => Ok(Some(Cow::Borrowed(HEART_OUTLINE))),
            "icons/heart-filled.svg" => Ok(Some(Cow::Borrowed(HEART_FILLED))),
            _ => gpui_component_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;

        if "icons/heart-outline.svg".starts_with(path) {
            assets.push("icons/heart-outline.svg".into());
        }
        if "icons/heart-filled.svg".starts_with(path) {
            assets.push("icons/heart-filled.svg".into());
        }

        Ok(assets)
    }
}
