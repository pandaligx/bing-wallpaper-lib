use crate::{model::WallpaperEntry, paths, settings::AppSettings};
use anyhow::{Context, Result, anyhow, ensure};
use fs2::FileExt;
use gosh_dl::{DownloadEngine, DownloadOptions, DownloadState, EngineConfig};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::OnceCell;

static ENGINE: OnceCell<Arc<DownloadEngine>> = OnceCell::const_new();

async fn engine() -> Result<&'static Arc<DownloadEngine>> {
    ENGINE
        .get_or_try_init(|| async {
            let root = paths::app_data_dir()?;
            let config = EngineConfig::default()
                .max_concurrent_downloads(4)
                .database_path(root.join("nsh-downloads.sqlite"));
            Ok(DownloadEngine::new(config).await?)
        })
        .await
}

pub fn validate_image(path: &Path) -> Result<()> {
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    ensure!(
        width > 0 && height > 0 && width <= 16384 && height <= 16384,
        "无效的图片尺寸"
    );
    ensure!(
        u64::from(width) * u64::from(height) <= 64 * 1024 * 1024,
        "图片超过解码限制"
    );
    image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .context("图片未完整下载")?;
    Ok(())
}

pub async fn download(entry: WallpaperEntry, settings: AppSettings) -> Result<PathBuf> {
    let root = settings.effective_download_dir()?;
    let name = entry.file_name();
    let mut final_path = root.join(&name);
    if final_path.with_extension("jpg.aria2").is_file() {
        final_path = final_path.with_extension("nsh.jpg");
    }
    let staging = root.join(".bing-downloads");
    std::fs::create_dir_all(&staging)?;
    let key = format!("{}-{:?}", entry.date, settings.download_resolution);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(staging.join(format!("{}.lock", entry.date)))?;
    lock.try_lock_exclusive().context("该日期已有下载任务")?;
    let url = entry.download_url(settings.download_resolution);
    let remote = url::Url::parse(&url)?;
    let host = remote.host_str().unwrap_or_default();
    ensure!(
        remote.scheme() == "https" && (host == "bing.com" || host.ends_with(".bing.com")),
        "无效的 Bing 图片地址"
    );
    let engine = engine().await?;
    let temporary = format!("{key}.part");
    let previous = engine.list().into_iter().find(|s| {
        s.metadata.url.as_deref() == Some(&url)
            && s.metadata.save_dir == staging
            && s.metadata.filename.as_deref() == Some(temporary.as_str())
            && !matches!(s.state, DownloadState::Completed)
    });
    let id = match previous {
        Some(task) => {
            if matches!(
                task.state,
                DownloadState::Error { .. } | DownloadState::Paused
            ) {
                engine.resume(task.id).await?;
            }
            task.id
        }
        None => {
            engine
                .add_http(
                    &url,
                    DownloadOptions::new()
                        .save_dir(&staging)
                        .filename(&temporary)
                        .max_connections(16),
                )
                .await?
        }
    };
    loop {
        let status = engine.status(id).ok_or_else(|| anyhow!("下载已取消"))?;
        match status.state {
            DownloadState::Completed => break,
            DownloadState::Error { message, .. } => return Err(anyhow!(message)),
            DownloadState::Paused => return Err(anyhow!("下载已暂停")),
            _ => tokio::time::sleep(Duration::from_millis(250)).await,
        }
    }
    let partial = staging.join(&temporary);
    tokio::task::spawn_blocking(move || {
        validate_image(&partial)?;
        std::fs::rename(&partial, &final_path).context("无法保存下载的壁纸")?;
        drop(lock);
        Ok(final_path)
    })
    .await?
}

pub fn progress() -> Result<String> {
    let mut rows = Vec::new();
    if let Some(engine) = ENGINE.get() {
        for job in engine.list() {
            rows.push(
                serde_json::json!({"id":job.id.to_string(), "name":job.metadata.name,
                "received":job.progress.completed_size, "total":job.progress.total_size,
                "speed":job.progress.download_speed,"state":job.state}),
            );
        }
    }
    Ok(serde_json::to_string(&rows)?)
}
