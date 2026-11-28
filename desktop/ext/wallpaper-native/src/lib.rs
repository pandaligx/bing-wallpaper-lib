//! Native services shared by the packaged desktop runners and NSH tests.
#[path = "../../../../src/archive.rs"]
pub mod archive;
mod catalog;
#[path = "../../../../src/i18n.rs"]
pub mod i18n;
mod installer;
mod integration;
pub use installer::supervise as install_update;
mod local;
#[path = "../../../../src/local_thumbnails.rs"]
pub mod local_thumbnails;
#[path = "../../../../src/model.rs"]
pub mod model;
pub mod paths;
mod platform;
mod preferences;
mod queue;
mod region;
mod registration;
mod release;
mod schedule;
pub use schedule::run as run_scheduled;
#[path = "../../../../src/settings.rs"]
pub mod settings;
mod transfer;
#[path = "../../../../src/wallpaper_selection.rs"]
mod wallpaper_selection;

use anyhow::{Context, Result as AppResult};
use nsh_core::prelude::*;
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
};
use tokio::runtime::Runtime;

#[cfg(windows)]
static PORTABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(windows)]
pub fn mark_portable() {
    PORTABLE.store(true, std::sync::atomic::Ordering::Release);
}

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

async fn disk<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> Result<T, String> {
    runtime()
        .map_err(message)?
        .spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(message)
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
    /// Detect a desktop provider that can set the wallpaper for this session.
    pub async fn can_apply_wallpaper() -> Result<bool, String> {
        disk(|| Ok(platform::available())).await
    }
    /// Drop unused decoded images while keeping leases for visible views.
    pub fn trim_images() {
        nsh_core::image::budget().clear();
    }
    /// Repair enabled legacy tasks after the portable GUI is successfully opened or moved.
    pub async fn repair_tasks() -> Result<(), String> {
        #[cfg(windows)]
        if PORTABLE.load(std::sync::atomic::Ordering::Acquire) {
            return disk(registration::repair_portable_tasks).await;
        }
        Ok(())
    }
    /// Read persisted task history from the application data directory.
    pub async fn download_history() -> Result<String, String> {
        disk(|| {
            match std::fs::read_to_string(paths::app_data_dir()?.join("download-history.json")) {
                Ok(text) => Ok(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("[]".into()),
                Err(e) => Err(e.into()),
            }
        })
        .await
    }
    /// Save the queue states after a transition, independently of engine segment data.
    pub async fn save_download_history(rows: String) -> Result<(), String> {
        disk(move || {
            anyhow::ensure!(rows.len() <= 32 * 1024 * 1024, "下载历史过大");
            paths::write_atomic(
                &paths::app_data_dir()?.join("download-history.json"),
                rows.as_bytes(),
            )
        })
        .await
    }
    /// Check the fixed-time task while the window is open, including after wake.
    pub async fn tick_daily() -> Result<String, String> {
        disk(|| {
            schedule::run("--daily-wallpaper")?;
            Ok(settings::AppSettings::load()
                .last_auto_wallpaper_date
                .map_or_else(String::new, |date| date.to_string()))
        })
        .await
    }
    /// Confirm that the upgraded UI can read and display its offline library.
    pub fn window_ready() {
        installer::acknowledge();
    }
    /// Report an installation failure to the restored application's window.
    pub async fn last_update_error() -> Result<String, String> {
        disk(installer::last_error).await
    }
    /// Prepare a verified installation and await the user's application exit.
    pub async fn install_update(version: String) -> Result<(), String> {
        #[cfg(windows)]
        if !PORTABLE.load(std::sync::atomic::Ordering::Acquire) {
            return Err("请从便携 exe 安装更新".into());
        }
        disk(move || installer::begin(&version)).await
    }
    /// Return the embedded desktop application version.
    pub fn app_version() -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }
    /// Check whether the desktop has a system tray host.
    pub fn tray_available() -> bool {
        integration::tray_available()
    }
    /// Tell the window whether this launch requested background mode.
    pub fn background_launch() -> bool {
        std::env::args().any(|a| a == "--background")
    }
    /// Return the operating system used for native integration.
    pub fn host_os() -> String {
        std::env::consts::OS.to_owned()
    }
    /// Configure login startup for the installed portable application.
    pub async fn configure_startup(enabled: bool) -> Result<(), String> {
        disk(move || integration::startup_enabled(enabled)).await
    }
    /// Open a project location or the configured download directory.
    pub async fn open_location(kind: String) -> Result<(), String> {
        disk(move || integration::open_location(&kind)).await
    }
    /// Show the desktop folder chooser and return an absolute path.
    pub async fn choose_folder() -> Result<String, String> {
        disk(integration::choose_folder).await
    }
    /// Return the effective wallpaper download directory.
    pub async fn download_folder() -> Result<String, String> {
        disk(|| {
            Ok(settings::AppSettings::load()
                .effective_download_dir()?
                .to_string_lossy()
                .into_owned())
        })
        .await
    }
    /// Apply the selected daily wallpaper source immediately.
    pub async fn change_now() -> Result<(), String> {
        disk(|| schedule::run("--change-now")).await
    }
    /// Translate the requested labels with the legacy six-language catalogue.
    pub fn labels(language: String, locale: String, keys: String) -> Result<String, String> {
        let selected: i18n::LanguagePreference =
            serde_json::from_value(serde_json::Value::String(language))
                .map_err(|e| e.to_string())?;
        let selected = if selected == i18n::LanguagePreference::System {
            i18n::LanguagePreference::from_locale(&locale)
        } else {
            selected
        };
        let keys: Vec<String> = serde_json::from_str(&keys).map_err(|e| e.to_string())?;
        let labels: std::collections::BTreeMap<_, _> = keys
            .iter()
            .map(|k| (k, i18n::translate(selected, k)))
            .collect();
        Ok(serde_json::json!({"locale":selected.gpui_locale(),"labels":labels}).to_string())
    }
    /// Read the corrected offline catalogue, newest date first.
    pub async fn catalog() -> Result<String, String> {
        runtime()
            .map_err(message)?
            .spawn_blocking(catalog::list)
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Changes when a refresh changes the rows or their metadata.
    pub fn catalog_revision() -> u32 {
        catalog::revision()
    }

    /// Merge the regional archive and recent official records into the local catalogue.
    pub async fn refresh() -> Result<String, String> {
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

    /// Check this region's release and authenticate its manifest before showing a newer version.
    pub async fn check_update(download: bool) -> Result<String, String> {
        let source = region::selected()
            .and_then(|s| s.context("请选择更新地区"))
            .map_err(message)?;
        runtime()
            .map_err(message)?
            .spawn(release::check(source, download))
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Saved region mode, including automatic selection independently of the resolved provider.
    pub fn region_preference() -> Result<String, String> {
        region::preference().map_err(message)
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
                let (path, reused) =
                    transfer::controlled_download(entry, settings, queue::control(&date)).await?;
                if apply {
                    let file = path.clone();
                    tokio::task::spawn_blocking(move || platform::apply(&file, &target)).await??;
                }
                Ok::<_, anyhow::Error>(serde_json::json!({"path":path,"reused":reused}).to_string())
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Reserve a date before the asynchronous download starts, including its metadata probe.
    pub fn begin_download(date: String) -> Result<(), String> {
        queue::begin(&date).map_err(message)
    }

    /// Release the UI worker's reservation after its completion has been handled.
    pub fn finish_download(date: String) {
        queue::finish(&date);
    }

    /// Stop a queued or active image. Cancelling a previously paused image discards its partial transfer.
    pub async fn stop_download(date: String, cancel: bool) -> Result<(), String> {
        if !queue::stop(&date, cancel).map_err(message)? && cancel {
            runtime()
                .map_err(message)?
                .spawn(async move { transfer::discard_paused(&date).await })
                .await
                .map_err(|e| e.to_string())?
                .map_err(message)?;
        }
        Ok(())
    }

    /// Snapshot transfer states and byte counters.
    pub fn progress() -> Result<String, String> {
        transfer::progress().map_err(message)
    }

    /// Stop an active transfer, keeping its resumable bytes.
    pub async fn pause(id: String) -> Result<(), String> {
        runtime()
            .map_err(message)?
            .spawn(async move { transfer::pause(&id).await })
            .await
            .map_err(|e| e.to_string())?
            .map_err(message)
    }

    /// Enumerate monitor IDs supported by the desktop wallpaper provider.
    pub fn monitors() -> Result<String, String> {
        platform::monitors().map_err(message)
    }

    /// Return the effective preferences, including values used by the legacy Windows app.
    pub async fn preferences() -> Result<String, String> {
        disk(preferences::get).await
    }

    /// Atomically merge changes, backing up the original settings on the first write.
    pub async fn configure(patch: String) -> Result<String, String> {
        disk(move || preferences::update(&patch)).await
    }

    /// Install or remove one user-level daily or repeating task and save its preferences.
    pub async fn configure_schedule(patch: String) -> Result<String, String> {
        disk(move || registration::configure(&patch)).await
    }

    /// List completed images in the user's existing download folder.
    pub async fn local_files() -> Result<String, String> {
        disk(local::files).await
    }
    /// Read the selected local file's dimensions and size on demand.
    pub async fn local_details(name: String) -> Result<String, String> {
        disk(move || local::details(&name)).await
    }

    /// Read a cached thumbnail or a preview bounded to 1080p.
    pub async fn local_image(name: String, preview: bool) -> Result<String, String> {
        disk(move || local::image(&name, preview)).await
    }

    /// Apply a completed local image without contacting a remote server.
    pub async fn apply_local(name: String) -> Result<(), String> {
        disk(move || local::apply(&name)).await
    }

    /// Delete a confirmed selection and report the result for each name.
    pub async fn delete_local(names: String) -> Result<String, String> {
        disk(move || local::delete(&names)).await
    }

    /// Remove only generated thumbnails.
    pub async fn clear_thumbnail_cache() -> Result<(), String> {
        disk(integration::clear_cache).await
    }
}
