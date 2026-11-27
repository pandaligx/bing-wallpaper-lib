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
            let mut config = EngineConfig::default()
                .max_concurrent_downloads(4)
                .database_path(root.join("nsh-downloads.sqlite"));
            config.http.max_redirects = 0;
            Ok(DownloadEngine::new(config).await?)
        })
        .await
}

fn verify_download_ending(path: &Path) -> Result<()> {
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    if reader.format() == Some(image::ImageFormat::Jpeg) {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(path)?;
        let mut marker = [0u8; 2];
        file.seek(SeekFrom::End(-2))?;
        file.read_exact(&mut marker)?;
        ensure!(marker == [0xff, 0xd9], "JPEG 文件未完整结束");
    }
    Ok(())
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
    let mut available = None;
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
            available.get_or_insert_with(|| path.clone());
            continue;
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
    available
        .map(|path| (path, false))
        .context("现有文件与下载目标冲突，已保留原文件")
}

pub async fn download(entry: WallpaperEntry, settings: AppSettings) -> Result<PathBuf> {
    Ok(download_with_status(entry, settings).await?.0)
}

pub async fn download_with_status(
    entry: WallpaperEntry,
    settings: AppSettings,
) -> Result<(PathBuf, bool)> {
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
        return Ok((final_path, true));
    }
    let url = entry.download_url(settings.download_resolution);
    let remote = url::Url::parse(&url)?;
    let host = remote.host_str().unwrap_or_default();
    ensure!(
        remote.scheme() == "https" && (host == "bing.com" || host.ends_with(".bing.com")),
        "无效的 Bing 图片地址"
    );
    let temporary = format!("{key}.part");
    static BING: std::sync::LazyLock<Result<reqwest::Client, reqwest::Error>> =
        std::sync::LazyLock::new(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(25))
                .redirect(reqwest::redirect::Policy::custom(|step| {
                    let url = step.url();
                    if step.previous().len() < 8
                        && url.scheme() == "https"
                        && url
                            .host_str()
                            .is_some_and(|h| h == "bing.com" || h.ends_with(".bing.com"))
                    {
                        step.follow()
                    } else {
                        step.error("Bing 图片重定向无效")
                    }
                }))
                .build()
        });
    let client = BING.as_ref().map_err(|e| anyhow!(e.to_string()))?;
    let origin = probe(client, &url, None).await?;
    let partial = fetch(&origin, &staging, &temporary).await?;
    tokio::task::spawn_blocking(move || {
        let temporary = tempfile::TempPath::try_from_path(partial)?;
        if let Some(bytes) = origin.length {
            ensure!(
                std::fs::metadata(&temporary)?.len() == bytes,
                "图片下载长度错误"
            );
        }
        verify_download_ending(&temporary)?;
        validate_image(&temporary)?;
        temporary
            .persist_noclobber(&final_path)
            .context("无法保存下载的壁纸")?;
        drop(lock);
        Ok((final_path, false))
    })
    .await?
}

pub(crate) async fn fetch(origin: &Origin, staging: &Path, temporary: &str) -> Result<PathBuf> {
    use std::{
        collections::BTreeMap,
        sync::{LazyLock, Mutex},
    };
    use tokio::sync::Semaphore;
    static HOSTS: LazyLock<Mutex<BTreeMap<String, Arc<Semaphore>>>> =
        LazyLock::new(|| Mutex::new(BTreeMap::new()));
    let host = origin.url.host_str().context("无效的下载主机")?;
    let slots = HOSTS
        .lock()
        .map_err(|_| anyhow!("连接池不可用"))?
        .entry(host.to_owned())
        .or_insert_with(|| Arc::new(Semaphore::new(16)))
        .clone();
    let _permit = slots.acquire_many(origin.connections as u32).await?;
    let url = origin.url.as_str();
    let engine = engine().await?;
    let previous = engine.list().into_iter().find(|s| {
        s.metadata.url.as_deref() == Some(url)
            && s.metadata.save_dir == staging
            && s.metadata.filename.as_deref() == Some(temporary)
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
                    url,
                    DownloadOptions::new()
                        .save_dir(staging)
                        .filename(temporary)
                        .max_connections(origin.connections),
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
    Ok(staging.join(temporary))
}

pub(crate) struct Origin {
    url: url::Url,
    length: Option<u64>,
    connections: usize,
}
/// Resolve redirects with the selected provider's policy, then test a real byte range.
pub(crate) async fn probe(
    client: &reqwest::Client,
    url: &str,
    expected: Option<u64>,
) -> Result<Origin> {
    use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
    let head = client.head(url).send().await?;
    let mut length = expected.or_else(|| {
        head.status()
            .is_success()
            .then(|| {
                head.headers()
                    .get(CONTENT_LENGTH)?
                    .to_str()
                    .ok()?
                    .parse()
                    .ok()
            })
            .flatten()
    });
    let mut final_url = head.url().clone();
    let mut connections = 1;
    if matches!(head.status().as_u16(), 403 | 405 | 501)
        || length.is_none_or(|size| size >= 2 * 1024 * 1024)
    {
        let reply = client
            .get(final_url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await?
            .error_for_status()?;
        final_url = reply.url().clone();
        if reply.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let size: u64 = reply
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|raw| raw.to_str().ok())
                .and_then(|s| s.strip_prefix("bytes 0-0/"))
                .context("服务器返回了错误的 Range")?
                .parse()?;
            ensure!(
                size > 0 && length.is_none_or(|n| n == size),
                "下载对象已改变"
            );
            length = Some(size);
            connections = (size / (1024 * 1024)).clamp(1, 16) as usize;
        } else if length.is_none() {
            length = reply
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok()?.parse().ok());
        }
    } else {
        head.error_for_status()?;
    }
    Ok(Origin {
        url: final_url,
        length,
        connections,
    })
}

pub fn progress() -> Result<String> {
    let mut rows = Vec::new();
    if let Some(engine) = ENGINE.get() {
        for job in engine.list() {
            rows.push(
                serde_json::json!({"id":job.id.to_string(), "date":job.metadata.filename.as_deref().and_then(|s| s.get(..10)), "update":job.metadata.filename.as_deref() == Some("update.partial"), "name":job.metadata.name,
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
    fn detects_real_range_support_and_changed_content() {
        use std::io::{BufRead, BufReader, Write};
        fn probe_fixture(range: &str) -> Result<Origin> {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let url = format!("http://{}/wallpaper", listener.local_addr()?);
            let range = range.to_owned();
            let server = std::thread::spawn(move || {
                for response in ["200 OK\r\nContent-Length: 4194304".to_owned(), range] {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut reader = BufReader::new(&stream);
                    let mut line = String::new();
                    while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                        line.clear();
                    }
                    write!(stream, "HTTP/1.1 {response}\r\nConnection: close\r\n\r\n").unwrap();
                }
            });
            let result = crate::runtime()?.block_on(probe(&reqwest::Client::new(), &url, None));
            server.join().unwrap();
            result
        }
        assert_eq!(
            probe_fixture(
                "206 Partial Content\r\nContent-Range: bytes 0-0/4194304\r\nContent-Length: 1"
            )
            .unwrap()
            .connections,
            4
        );
        assert_eq!(
            probe_fixture("200 OK\r\nContent-Length: 4194304")
                .unwrap()
                .connections,
            1
        );
        assert!(
            probe_fixture(
                "206 Partial Content\r\nContent-Range: bytes 0-0/4194303\r\nContent-Length: 1"
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "downloads one real Bing wallpaper into an isolated temporary user directory"]
    fn fetches_validates_and_reuses_a_real_bing_wallpaper() {
        let root = tempfile::tempdir().unwrap();
        let entry = crate::archive::bundled_entries()
            .into_iter()
            .find(|e| e.date.to_string() == "2026-11-26")
            .unwrap();
        let settings = AppSettings {
            download_dir: Some(root.path().join("images")),
            download_resolution: DownloadResolution::OneK,
            ..Default::default()
        };
        crate::runtime().unwrap().block_on(async {
            let mut config = EngineConfig::default().database_path(root.path().join("test.sqlite"));
            config.http.max_redirects = 0;
            let engine = DownloadEngine::new(config).await.unwrap();
            assert!(ENGINE.set(engine).is_ok());
            let output = download(entry.clone(), settings.clone()).await.unwrap();
            assert_eq!(validate_image(&output).unwrap(), (1920, 1080));
            let stamp = output.metadata().unwrap().modified().unwrap();
            assert_eq!(download(entry, settings).await.unwrap(), output);
            assert_eq!(output.metadata().unwrap().modified().unwrap(), stamp);
            ENGINE.get().unwrap().shutdown().await.unwrap();
        });
    }

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
        // A completed alternate remains usable if the old transfer was cleared elsewhere.
        std::fs::remove_file(original.with_extension("jpg.aria2")).unwrap();
        std::fs::remove_file(&original).unwrap();
        assert_eq!(
            destination(temp.path(), &entry, DownloadResolution::OneK).unwrap(),
            (original.with_extension("1k.jpg"), true)
        );
    }
}
