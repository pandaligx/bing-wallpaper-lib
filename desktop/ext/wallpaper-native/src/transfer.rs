use crate::{
    model::WallpaperEntry,
    paths,
    settings::{AppSettings, DownloadResolution},
};
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

pub fn validate_image(path: &Path) -> Result<(u32, u32)> {
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
    Ok((width, height))
}

/// Pick a destination without replacing an existing original or an aria2 partial.
fn destination(
    root: &Path,
    entry: &WallpaperEntry,
    resolution: DownloadResolution,
) -> Result<(PathBuf, bool)> {
    let original = root.join(entry.file_name());
    let suffix = match resolution {
        DownloadResolution::Original => "uhd",
        DownloadResolution::FourK => "4k",
        DownloadResolution::TwoK => "2k",
        DownloadResolution::OneK => "1k",
    };
    let alternate = original.with_extension(format!("{suffix}.jpg"));
    for (path, known_resolution) in [(&original, false), (&alternate, true)] {
        let mut marker = path.as_os_str().to_owned();
        marker.push(".aria2");
        if Path::new(&marker).try_exists()? {
            continue;
        }
        if !path.try_exists()? {
            if resolution == DownloadResolution::Original && !known_resolution {
                continue;
            }
            return Ok((path.clone(), false));
        }
        if let Ok(dimensions) = validate_image(path) {
            let matches = match resolution {
                DownloadResolution::FourK => dimensions == (3840, 2160),
                DownloadResolution::TwoK => dimensions == (2560, 1440),
                DownloadResolution::OneK => dimensions == (1920, 1080),
                DownloadResolution::Original => false,
            };
            if known_resolution || matches {
                return Ok((path.clone(), true));
            }
        }
    }
    anyhow::bail!("现有文件与下载目标冲突，已保留原文件")
}

pub async fn download(entry: WallpaperEntry, settings: AppSettings) -> Result<PathBuf> {
    let root = settings.effective_download_dir()?;
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
    let resolution = settings.download_resolution;
    let (entry, final_path, existing) = tokio::task::spawn_blocking(move || {
        let (path, existing) = destination(&root, &entry, resolution)?;
        Ok::<_, anyhow::Error>((entry, path, existing))
    })
    .await??;
    if existing {
        return Ok(final_path);
    }
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
        tempfile::TempPath::try_from_path(partial)?
            .persist_noclobber(&final_path)
            .context("无法保存下载的壁纸")?;
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
                serde_json::json!({"id":job.id.to_string(), "date":job.metadata.filename.as_deref().and_then(|s| s.get(..10)), "name":job.metadata.name,
                "received":job.progress.completed_size, "total":job.progress.total_size,
                "speed":job.progress.download_speed,"state":job.state.to_aria2_status()}),
            );
        }
    }
    Ok(serde_json::to_string(&rows)?)
}

pub async fn pause(id: &str) -> Result<()> {
    let engine = engine().await?;
    let job = engine
        .list()
        .into_iter()
        .find(|job| job.id.to_string() == id)
        .context("找不到下载任务")?;
    engine.pause(job.id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria2_partial_and_existing_original_survive_repeated_selection() {
        let temp = tempfile::tempdir().unwrap();
        let entry = crate::archive::bundled_entries().pop().unwrap();
        let original = temp.path().join(entry.file_name());
        std::fs::write(&original, b"previous download").unwrap();
        std::fs::write(original.with_extension("jpg.aria2"), b"resume").unwrap();
        let (path, ready) = destination(temp.path(), &entry, DownloadResolution::OneK).unwrap();
        assert!(!ready);
        assert_eq!(path, original.with_extension("1k.jpg"));
        image::RgbImage::new(8, 8).save(&path).unwrap();
        assert_eq!(
            destination(temp.path(), &entry, DownloadResolution::OneK).unwrap(),
            (path, true)
        );
        assert_eq!(std::fs::read(&original).unwrap(), b"previous download");
        assert!(original.with_extension("jpg.aria2").is_file());
    }
}
