//! Native services shared by the packaged desktop runners and NSH tests.
#[path = "../../../../src/archive.rs"]
pub mod archive;
mod catalog;
#[path = "../../../../src/i18n.rs"]
pub mod i18n;
#[path = "../../../../src/model.rs"]
pub mod model;
pub mod paths;
mod platform;
mod region;
#[path = "../../../../src/settings.rs"]
pub mod settings;
mod transfer;

use anyhow::{Context, Result as AppResult};
use nsh_core::prelude::*;
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};
use tokio::runtime::Runtime;

fn runtime() -> AppResult<&'static Runtime> {
    static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| Runtime::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
fn message(error: anyhow::Error) -> String {
    format!("{error:#}")
}

fn favorites() -> AppResult<BTreeSet<chrono::NaiveDate>> {
    match std::fs::read(paths::favorites_file()?) {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(e) => Err(e.into()),
    }
}

#[native_module("wallpaper-native")]
fn native(registry: &mut Registry) {
    registry.module::<Library>();
}

#[module]
/// Wallpaper catalogue, verified downloads, monitor selection and local preferences.
pub struct Library;

#[module]
impl Library {
    /// Read the corrected offline catalogue, newest date first.
    pub async fn catalog() -> Result<String, String> {
        runtime()
            .map_err(message)?
            .spawn_blocking(catalog::list)
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Merge the regional archive and recent official records into the local catalogue.
    pub async fn refresh() -> Result<(), String> {
        let source = region::selected()
            .and_then(|s| s.context("请选择更新地区"))
            .map_err(message)?;
        runtime()
            .map_err(message)?
            .spawn(catalog::refresh(source))
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Selected release provider, or an empty string when the region is unknown.
    pub fn source() -> Result<String, String> {
        Ok(region::selected()
            .map_err(message)?
            .map_or("", |s| s.name())
            .to_owned())
    }

    /// Persist the user's automatic, mainland or international region selection.
    pub fn set_source(value: String) -> Result<(), String> {
        region::select(&value).map_err(message)
    }

    /// List the user's saved dates, including dates stored by earlier releases.
    pub fn favorites() -> Result<String, String> {
        serde_json::to_string(&favorites().map_err(message)?).map_err(|e| e.to_string())
    }

    /// Toggle one date and return the persisted list.
    pub fn favorite(date: String) -> Result<String, String> {
        static LOCK: Mutex<()> = Mutex::new(());
        let _guard = LOCK.lock().map_err(|_| "收藏不可用".to_owned())?;
        let entry = catalog::entry(&date).map_err(message)?;
        let mut dates = favorites().map_err(message)?;
        if !dates.insert(entry.date) {
            dates.remove(&entry.date);
        }
        let text = serde_json::to_string(&dates).map_err(|e| e.to_string())?;
        paths::write_atomic(&paths::favorites_file().map_err(message)?, text.as_bytes())
            .map_err(message)?;
        Ok(text)
    }

    /// Download and validate the selected resolution, optionally setting it as the desktop.
    pub async fn download(date: String, apply: bool) -> Result<String, String> {
        let entry = catalog::entry(&date).map_err(message)?;
        runtime()
            .map_err(message)?
            .spawn(async move {
                let settings = settings::AppSettings::load();
                let target = settings.wallpaper_target.clone();
                let path = transfer::download(entry, settings).await?;
                if apply {
                    let file = path.clone();
                    tokio::task::spawn_blocking(move || platform::apply(&file, &target)).await??;
                }
                Ok::<_, anyhow::Error>(path.to_string_lossy().into_owned())
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Snapshot transfer states and byte counters.
    pub fn progress() -> Result<String, String> {
        transfer::progress().map_err(message)
    }

    /// Enumerate monitor IDs supported by the desktop wallpaper provider.
    pub fn monitors() -> Result<String, String> {
        platform::monitors().map_err(message)
    }
}
