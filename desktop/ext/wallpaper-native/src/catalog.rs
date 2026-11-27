use crate::{archive, model::WallpaperEntry, paths, region::Source};
use anyhow::{Context, Result, anyhow, ensure};
use std::sync::{LazyLock, RwLock};

static CATALOG: LazyLock<RwLock<Vec<WallpaperEntry>>> =
    LazyLock::new(|| RwLock::new(archive::local_entries()));

pub fn entry(date: &str) -> Result<WallpaperEntry> {
    let date: chrono::NaiveDate = date.parse()?;
    let entries = CATALOG.read().map_err(|_| anyhow!("壁纸库不可用"))?;
    let index = entries
        .binary_search_by(|item| date.cmp(&item.date))
        .map_err(|_| anyhow!("该日期不存在"))?;
    Ok(entries[index].clone())
}

pub fn list() -> Result<String> {
    let entries = CATALOG.read().map_err(|_| anyhow!("壁纸库不可用"))?;
    let data: Vec<_> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "date":e.date, "headline":e.headline, "description":e.title,
                "thumbnail":e.thumbnail_url(),"preview":e.preview_url()
            })
        })
        .collect();
    Ok(serde_json::to_string(&data)?)
}

async fn text(client: &reqwest::Client, url: &str) -> Result<String> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            data.len() + chunk.len() <= 32 * 1024 * 1024,
            "归档超过大小限制"
        );
        data.extend_from_slice(&chunk);
    }
    String::from_utf8(data).context("归档不是 UTF-8")
}

pub async fn refresh(source: Source) -> Result<String> {
    static REFRESH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _lock = REFRESH.lock().await;
    if crate::region::selected()? != Some(source) {
        return Ok(String::new());
    }
    let client = reqwest::Client::builder()
        .user_agent("BingWallpaperLib/0.3")
        .timeout(std::time::Duration::from_secs(18))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            let bing = |url: &reqwest::Url| {
                url.scheme() == "https"
                    && url
                        .host_str()
                        .is_some_and(|h| h == "bing.com" || h.ends_with(".bing.com"))
            };
            let permitted = match attempt.previous().first() {
                Some(first) if bing(first) => bing(attempt.url()),
                _ => crate::release::allowed(source, attempt.url()),
            };
            if permitted && attempt.previous().len() < 8 {
                attempt.follow()
            } else {
                attempt.error("归档重定向不属于所选来源")
            }
        }))
        .build()?;
    let remote = text(&client, source.archive()).await;
    let mut entries = archive::local_entries();
    let mut updated = false;
    let mut errors = Vec::new();
    match remote {
        Ok(json) => {
            let data = archive::parse_zxyongyo_archive(&json);
            if !data.is_empty() {
                entries = archive::merge_entries_prefer_primary(data, entries);
                updated = true;
            } else {
                errors.push(format!("{} 归档为空", source.name()));
            }
        }
        Err(e) => errors.push(format!("{} 归档：{e}", source.name())),
    }
    for host in ["cn.bing.com", "www.bing.com"] {
        let mut recent = Vec::new();
        for idx in [0, 8] {
            let url = format!(
                "https://{host}/HPImageArchive.aspx?format=js&idx={idx}&n=8&mkt=zh-CN&cc=cn"
            );
            match text(&client, &url)
                .await
                .and_then(|s| archive::parse_bing_window(&s))
            {
                Ok(rows) => recent.extend(rows),
                Err(e) => errors.push(format!("{host} 最近记录：{e}")),
            }
        }
        if !recent.is_empty() {
            entries = archive::merge_entries_prefer_primary(recent, entries);
            updated = true;
            break;
        }
    }
    ensure!(updated, "{}", errors.join(" / "));
    let entries = archive::apply_bundled_corrections(entries);
    if crate::region::selected()? != Some(source) {
        return Ok(String::new());
    }
    paths::write_atomic(&paths::cache_file()?, &serde_json::to_vec(&entries)?)?;
    *CATALOG.write().map_err(|_| anyhow!("壁纸库不可用"))? = entries;
    Ok(errors.join(" / "))
}
