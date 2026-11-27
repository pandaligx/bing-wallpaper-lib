//! Windows 任务计划启动的无界面单次换壁纸流程。

use crate::downloader::Aria2Manager;
use crate::model::WallpaperEntry;
use crate::settings::{AppSettings, WallpaperTarget};
use crate::wallpaper_selection::select_entry;
use anyhow::{bail, Context, Result};
use chrono::Local;
use http_client::HttpClient;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOWNLOAD_POLL_INTERVAL: Duration = Duration::from_millis(300);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

async fn download_entry(
    http: Arc<dyn HttpClient>,
    entry: &WallpaperEntry,
    settings: &AppSettings,
) -> Result<std::path::PathBuf> {
    let manager = Aria2Manager::start(http).await?;
    let filename = entry.file_name();
    let url = entry.download_url(settings.download_resolution);
    let gid = manager.add_uri(&url, &filename).await?;
    let started = Instant::now();

    loop {
        if started.elapsed() >= DOWNLOAD_TIMEOUT {
            bail!("计划任务下载壁纸超时");
        }

        let status = manager.tell_status(&gid).await?;
        match status
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
        {
            "complete" => {
                let path = settings.effective_download_dir()?.join(filename);
                manager.shutdown().await;
                if !path.exists() {
                    bail!("aria2 报告下载完成，但壁纸文件不存在: {}", path.display());
                }
                return Ok(path);
            }
            "error" | "removed" => {
                let message = status
                    .get("errorMessage")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("未知错误");
                bail!("计划任务下载壁纸失败: {message}");
            }
            _ => {
                smol::Timer::after(DOWNLOAD_POLL_INTERVAL).await;
            }
        }
    }
}

/// 执行一次周期壁纸任务。成功或失败后均由调用方直接结束本次无界面进程。
pub async fn run_once(http: Arc<dyn HttpClient>) -> Result<()> {
    let mut settings = AppSettings::load();
    if !settings.periodic_task_enabled {
        return Ok(());
    }

    let entries = crate::fetcher::fetch_all(http.clone())
        .await
        .context("计划任务加载壁纸列表失败")?;
    if entries.is_empty() {
        bail!("没有可用于周期任务的壁纸");
    }
    if let Err(err) = crate::fetcher::save_cache(&entries) {
        log::warn!("周期任务写入壁纸缓存失败: {err}");
    }

    let today = Local::now().date_naive();
    let favorites = crate::favorites::load();
    let selection =
        select_entry(&entries, &favorites, &settings, today).context("没有可用于周期任务的壁纸")?;
    if selection.fell_back_from_favorites {
        log::warn!("周期任务选择了随机收藏，但收藏为空，已回退到随机历史壁纸");
    }

    let path = download_entry(http, &selection.entry, &settings).await?;
    match &settings.wallpaper_target {
        WallpaperTarget::All => crate::wallpaper_setter::set_wallpaper_for_all_monitors(&path)?,
        WallpaperTarget::Monitor(monitor_id) => {
            crate::wallpaper_setter::set_wallpaper_for_monitor(&path, monitor_id)?
        }
    }

    if selection.used_daily_latest {
        settings.last_periodic_latest_date = Some(today);
        settings
            .save()
            .context("保存周期任务每日首张执行日期失败")?;
    }
    Ok(())
}
