//! GPUI host networking for the legacy Windows release.
use crate::model::WallpaperEntry;
use anyhow::{bail, Context, Result};
use futures::AsyncReadExt;
use http_client::{HttpClient, HttpRequestExt, Request};
use std::sync::Arc;
use crate::archive::{apply_bundled_corrections, archive_covers_seed, dedup_by_date, merge_entries_prefer_primary, parse_bing_window, parse_zxyongyo_archive};
pub use crate::archive::{bundled_entries, has_new_entry, load_cache, local_entries, save_cache};
const BING_API_SOURCE_URLS: &[&str] = &[
    "https://cn.bing.com/HPImageArchive.aspx",
    "https://www.bing.com/HPImageArchive.aspx",
];

/// Bing 官方接口最多稳定返回最近两个窗口；更大的 idx 会被服务端折回最近数据。
const BING_API_IDXS: &[u32] = &[0, 8];

const ZXYONGYO_ARCHIVE_URLS: &[&str] = &[
    "https://gitee.com/pandaligx/bing-wallpaper-lib/raw/main/assets/data/zxyongyo-bing-wallpaper.json",
    "https://cdn.jsdelivr.net/gh/zxyongyo/bing-daily-wallpaper@master/map.json",
    "https://fastly.jsdelivr.net/gh/zxyongyo/bing-daily-wallpaper@master/map.json",
    "https://raw.githubusercontent.com/zxyongyo/bing-daily-wallpaper/master/map.json",
];

const MAX_REMOTE_JSON_BYTES: u64 = 32 * 1024 * 1024;

/// 从 Bing 官方接口拉取最近窗口数据。
async fn fetch_recent_from_bing_api(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let mut last_err = None;

    for &source in BING_API_SOURCE_URLS {
        match fetch_recent_from_bing_source(http.clone(), source).await {
            Ok(entries) => return Ok(entries),
            Err(err) => {
                log::warn!("从 Bing 官方接口 {source} 获取壁纸列表失败: {err}");
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("没有可用的 Bing 官方壁纸接口")))
}

async fn fetch_recent_from_bing_source(
    http: Arc<dyn HttpClient>,
    source: &str,
) -> Result<Vec<WallpaperEntry>> {
    let mut entries = Vec::new();
    for idx in BING_API_IDXS {
        let url = format!("{source}?format=js&idx={idx}&n=8&mkt=zh-CN&cc=cn");
        let text = fetch_text(http.clone(), &url).await?;
        entries.extend(parse_bing_window(&text)?);
    }

    let mut entries = dedup_by_date(entries);
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.date));
    if entries.is_empty() {
        bail!("Bing 官方接口返回了空壁纸列表");
    }
    Ok(entries)
}

async fn fetch_zxyongyo_archive(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let mut last_err = None;

    for &url in ZXYONGYO_ARCHIVE_URLS {
        match fetch_text(http.clone(), url).await {
            Ok(text) => {
                let entries = parse_zxyongyo_archive(&text);
                if entries.is_empty() {
                    last_err = Some(anyhow::anyhow!("zxyongyo 归档为空"));
                    continue;
                }
                return Ok(entries);
            }
            Err(err) => {
                log::warn!("从 zxyongyo 归档 {url} 获取壁纸列表失败: {err}");
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("没有可用的 zxyongyo 壁纸归档")))
}

/// 拉取 zxyongyo 历史归档与 Bing 官方最近窗口，合并后按日期倒序返回。
pub async fn fetch_all(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let local_seed = local_entries();
    let archive = match fetch_zxyongyo_archive(http.clone()).await {
        Ok(entries) if archive_covers_seed(&entries, &local_seed) => {
            merge_entries_prefer_primary(entries, local_seed)
        }
        Ok(entries) => {
            log::warn!(
                "zxyongyo 历史归档疑似不完整（远端 {} 张，本地 {} 张），合并并保留本地历史数据",
                entries.len(),
                local_seed.len()
            );
            merge_entries_prefer_primary(entries, local_seed)
        }
        Err(err) => {
            log::warn!("zxyongyo 历史归档不可用，使用本地缓存或内置快照: {err}");
            local_seed
        }
    };
    // 远程源尚未同步或仍含旧错误时，也必须以本地核验过的 corrections 为准。
    let archive = apply_bundled_corrections(archive);

    match fetch_recent_from_bing_api(http).await {
        Ok(recent) => Ok(merge_entries_prefer_primary(recent, archive)),
        Err(err) if archive.is_empty() => Err(err),
        Err(err) => {
            log::warn!("Bing 官方最近窗口不可用，仅使用 zxyongyo/本地历史归档: {err}");
            Ok(archive)
        }
    }
}

/// 请求单个 URL 并返回响应文本内容（不做任何解析，供 `fetch_all` 依次尝试候选源时复用）。
async fn fetch_text(http: Arc<dyn HttpClient>, url: &str) -> Result<String> {
    let request = Request::get(url)
        .follow_redirects(http_client::RedirectPolicy::FollowAll)
        .body(Default::default())
        .context("构建请求失败")?;
    let mut response = http.send(request).await.context("请求失败")?;

    if !response.status().is_success() {
        bail!("返回了错误状态码: {}", response.status());
    }

    let mut body = Vec::new();
    response
        .body_mut()
        .take(MAX_REMOTE_JSON_BYTES + 1)
        .read_to_end(&mut body)
        .await
        .context("读取响应内容失败")?;
    if body.len() as u64 > MAX_REMOTE_JSON_BYTES {
        bail!("响应内容超过大小限制");
    }
    String::from_utf8(body).context("响应内容不是合法的 UTF-8 文本")
}
