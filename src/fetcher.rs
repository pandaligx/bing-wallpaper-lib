//! 抓取 Bing 官方首页壁纸 API 与 zxyongyo/bing-daily-wallpaper 历史归档。
//!
//! Bing 官方 `HPImageArchive.aspx` 接口只提供最近一小段历史；完整历史优先使用
//! `zxyongyo/bing-daily-wallpaper` 的 `map.json`，首次安装或网络不可用时使用随 exe
//! 内置的同格式 JSON 快照兜底。

use crate::model::WallpaperEntry;
use crate::paths::cache_file;
use anyhow::{bail, Context, Result};
use chrono::{Days, NaiveDate};
use futures::AsyncReadExt;
use http_client::{HttpClient, HttpRequestExt, Request};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

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

const BUNDLED_ZXYONGYO_ARCHIVE: &str = include_str!("../assets/data/zxyongyo-bing-wallpaper.json");
const MAX_REMOTE_JSON_BYTES: u64 = 32 * 1024 * 1024;

/// 去重：同一天可能存在两条记录（历史上偶发现象），保留第一条（列表中最靠前的一条）。
pub fn dedup_by_date(entries: Vec<WallpaperEntry>) -> Vec<WallpaperEntry> {
    let mut seen: HashSet<NaiveDate> = HashSet::new();
    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        if seen.insert(entry.date) {
            result.push(entry);
        }
    }
    result
}

/// 合并 Bing 官方近期列表与 zxyongyo 归档。
///
/// 同一日期优先使用官方记录；同一图片被两边标成不同日期时，以归档日期为准。
/// Bing 官方接口的日期边界偶尔会比中国区实际展示日期早一天，不能因此删除归档中
/// 日期正确的记录。
fn merge_entries_prefer_primary(
    primary: Vec<WallpaperEntry>,
    fallback: Vec<WallpaperEntry>,
) -> Vec<WallpaperEntry> {
    let mut entries = dedup_by_date(fallback);

    for entry in dedup_by_date(primary) {
        let same_image_index = image_key(&entry).and_then(|key| {
            entries
                .iter()
                .position(|existing| image_key(existing).as_deref() == Some(key.as_str()))
        });

        if let Some(index) = same_image_index {
            if entries[index].date == entry.date {
                entries[index] = entry;
            }
            continue;
        }

        if let Some(index) = entries
            .iter()
            .position(|existing| existing.date == entry.date)
        {
            entries[index] = entry;
        } else {
            entries.push(entry);
        }
    }

    entries.sort_by_key(|entry| std::cmp::Reverse(entry.date));
    entries
}

fn image_key(entry: &WallpaperEntry) -> Option<String> {
    let start = entry.url.find("OHR.")? + "OHR.".len();
    let rest = &entry.url[start..];
    let end = rest.find(['_', '.', '&', '?']).unwrap_or(rest.len());
    let key = rest[..end].trim();
    (!key.is_empty()).then(|| key.to_string())
}

/// 读取随 exe 内置的壁纸列表快照（已按日期倒序）。
pub fn bundled_entries() -> Vec<WallpaperEntry> {
    parse_zxyongyo_archive(BUNDLED_ZXYONGYO_ARCHIVE)
}

#[derive(Debug, Deserialize)]
struct BingArchiveResponse {
    images: Vec<BingArchiveImage>,
}

#[derive(Debug, Deserialize)]
struct BingArchiveImage {
    startdate: String,
    #[serde(default)]
    enddate: Option<String>,
    urlbase: String,
    copyright: String,
    #[serde(default)]
    copyrightlink: Option<String>,
    #[serde(default)]
    title: String,
}

#[derive(Debug, Deserialize)]
struct ZxyongyoArchiveResponse {
    images: Vec<ZxyongyoArchiveImage>,
}

#[derive(Debug, Deserialize)]
struct ZxyongyoArchiveImage {
    date: String,
    title: String,
    copyright: String,
    url_4k: String,
}

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
        let response: BingArchiveResponse =
            serde_json::from_str(&text).context("解析 Bing 官方壁纸 JSON 失败")?;
        entries.extend(response.images.into_iter().filter_map(bing_image_to_entry));
    }

    let mut entries = dedup_by_date(entries);
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.date));
    if entries.is_empty() {
        bail!("Bing 官方接口返回了空壁纸列表");
    }
    Ok(entries)
}

fn bing_image_to_entry(image: BingArchiveImage) -> Option<WallpaperEntry> {
    let start_date = NaiveDate::parse_from_str(&image.startdate, "%Y%m%d").ok()?;
    let date = image
        .enddate
        .as_deref()
        .and_then(|value| NaiveDate::parse_from_str(value, "%Y%m%d").ok())
        .unwrap_or_else(|| {
            start_date
                .checked_add_days(Days::new(1))
                .unwrap_or(start_date)
        });
    Some(WallpaperEntry {
        date,
        headline: non_empty_string(image.title),
        title: image.copyright.trim().to_string(),
        url: bing_uhd_url(&image.urlbase),
        copyright_link: image.copyrightlink.and_then(non_empty_string),
    })
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

fn parse_zxyongyo_archive(text: &str) -> Vec<WallpaperEntry> {
    let Ok(response) = serde_json::from_str::<ZxyongyoArchiveResponse>(text) else {
        return Vec::new();
    };
    let mut entries = dedup_by_date(
        response
            .images
            .into_iter()
            .filter_map(zxyongyo_image_to_entry)
            .collect(),
    );
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.date));
    entries
}

fn zxyongyo_image_to_entry(image: ZxyongyoArchiveImage) -> Option<WallpaperEntry> {
    let date = NaiveDate::parse_from_str(&image.date, "%Y-%m-%d").ok()?;
    Some(WallpaperEntry {
        date,
        headline: non_empty_string(image.title),
        title: image.copyright.trim().to_string(),
        url: image.url_4k.trim().to_string(),
        copyright_link: None,
    })
}

fn bing_uhd_url(urlbase: &str) -> String {
    let urlbase = urlbase.trim();
    let path = if urlbase.starts_with("http://") || urlbase.starts_with("https://") {
        urlbase.to_string()
    } else if urlbase.starts_with('/') {
        format!("https://cn.bing.com{urlbase}")
    } else {
        format!("https://cn.bing.com/{urlbase}")
    };
    format!("{path}_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4")
}

fn non_empty_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn local_seed_entries() -> Vec<WallpaperEntry> {
    let bundled = bundled_entries();
    load_cache()
        .ok()
        .flatten()
        .filter(|entries| !entries.is_empty())
        .map(|cached| merge_entries_prefer_primary(cached, bundled.clone()))
        .unwrap_or(bundled)
}

fn archive_covers_seed(archive: &[WallpaperEntry], seed: &[WallpaperEntry]) -> bool {
    if seed.is_empty() {
        return !archive.is_empty();
    }
    let archive_oldest = archive.iter().map(|entry| entry.date).min();
    let seed_oldest = seed.iter().map(|entry| entry.date).min();
    let keeps_history =
        matches!((archive_oldest, seed_oldest), (Some(remote), Some(local)) if remote <= local);
    let keeps_expected_count = archive.len().saturating_mul(10) >= seed.len().saturating_mul(9);
    keeps_history && keeps_expected_count
}

/// 拉取 zxyongyo 历史归档与 Bing 官方最近窗口，合并后按日期倒序返回。
pub async fn fetch_all(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let local_seed = local_seed_entries();
    let archive = match fetch_zxyongyo_archive(http.clone()).await {
        Ok(entries) if archive_covers_seed(&entries, &local_seed) => entries,
        Ok(entries) => {
            log::warn!(
                "zxyongyo 历史归档疑似不完整（远端 {} 张，本地 {} 张），继续使用本地历史数据",
                entries.len(),
                local_seed.len()
            );
            local_seed
        }
        Err(err) => {
            log::warn!("zxyongyo 历史归档不可用，使用本地缓存或内置快照: {err}");
            local_seed
        }
    };

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

/// 从本地缓存加载壁纸列表（如果存在）。
pub fn load_cache() -> Result<Option<Vec<WallpaperEntry>>> {
    let path = cache_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&path).context("读取本地壁纸缓存失败")?;
    let entries: Vec<WallpaperEntry> =
        serde_json::from_str(&data).context("解析本地壁纸缓存 JSON 失败")?;
    Ok(Some(entries))
}

/// 将壁纸列表写入本地缓存。
pub fn save_cache(entries: &[WallpaperEntry]) -> Result<()> {
    let path = cache_file()?;
    let data = serde_json::to_string_pretty(entries).context("序列化壁纸列表失败")?;
    std::fs::write(&path, data).context("写入本地壁纸缓存失败")?;
    Ok(())
}

/// 判断远端最新一条记录的日期，是否比本地缓存中已知的最新日期更新。
pub fn has_new_entry(cached: &[WallpaperEntry], fetched: &[WallpaperEntry]) -> bool {
    let cached_latest = cached.iter().map(|e| e.date).max();
    let fetched_latest = fetched.iter().map(|e| e.date).max();
    match (cached_latest, fetched_latest) {
        (Some(c), Some(f)) => f > c,
        (None, Some(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_zxyongyo_archive_json() {
        let text = r#"{"images":[{"hsh":"490c","date":"2026-07-04","title":"紫色花海","copyright":"瓦朗索勒高原的薰衣草行，普罗旺斯，法国 (© Robert Harding/Shutterstock)","url_preview":"https://example.com/preview.jpg","url_1080":"https://example.com/1080.jpg","url_4k":"https://example.com/4k.jpg"},{"hsh":"4504","date":"2026-07-03","title":"此行，不虚绕道","copyright":"凯泽斯堡，阿尔萨斯，法国 (© Federica Gentile/Getty Images)","url_preview":"https://example.com/preview2.jpg","url_1080":"https://example.com/1080-2.jpg","url_4k":"https://example.com/4k-2.jpg"}]}"#;
        let entries = parse_zxyongyo_archive(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()
        );
        assert_eq!(entries[0].headline.as_deref(), Some("紫色花海"));
        assert!(entries[0].title.contains("瓦朗索勒高原"));
        assert_eq!(entries[0].url, "https://example.com/4k.jpg");
    }

    #[test]
    fn bundled_snapshot_is_parseable() {
        let entries = bundled_entries();
        assert!(!entries.is_empty());
        assert!(entries.windows(2).all(|pair| pair[0].date >= pair[1].date));

        let april_2020: Vec<_> = entries
            .iter()
            .filter(|entry| {
                entry.date >= NaiveDate::from_ymd_opt(2020, 4, 1).unwrap()
                    && entry.date <= NaiveDate::from_ymd_opt(2020, 4, 30).unwrap()
            })
            .collect();
        assert_eq!(april_2020.len(), 30);
        assert!(april_2020
            .iter()
            .any(|entry| entry.date == NaiveDate::from_ymd_opt(2020, 4, 4).unwrap()));
    }

    #[test]
    fn merges_primary_entries_with_fallback_by_date() {
        let primary = vec![WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            headline: Some("中文标题".to_string()),
            title: "中文地点 (© A/B)".to_string(),
            url: "https://cn.bing.com/th?id=OHR.Today_ZH-CN_UHD.jpg".to_string(),
            copyright_link: None,
        }];
        let fallback = vec![
            WallpaperEntry {
                date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
                headline: Some("English title".to_string()),
                title: "English place (© A/B)".to_string(),
                url: "https://cn.bing.com/th?id=OHR.Today_EN-US_UHD.jpg".to_string(),
                copyright_link: None,
            },
            WallpaperEntry {
                date: NaiveDate::from_ymd_opt(2021, 2, 1).unwrap(),
                headline: Some("Old title".to_string()),
                title: "Old place (© A/B)".to_string(),
                url: "https://cn.bing.com/th?id=OHR.Old_ZH-CN_UHD.jpg".to_string(),
                copyright_link: None,
            },
        ];

        let entries = merge_entries_prefer_primary(primary, fallback);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()
        );
        assert_eq!(entries[0].headline.as_deref(), Some("中文标题"));
        assert!(entries[0].url.contains("_ZH-CN"));
        assert_eq!(
            entries[1].date,
            NaiveDate::from_ymd_opt(2021, 2, 1).unwrap()
        );
        assert_eq!(entries[1].headline.as_deref(), Some("Old title"));
    }

    #[test]
    fn dedups_duplicate_dates_keeping_first() {
        let entries = dedup_by_date(vec![
            WallpaperEntry {
                date: NaiveDate::from_ymd_opt(2025, 4, 10).unwrap(),
                headline: Some("A".to_string()),
                title: "A".to_string(),
                url: "https://example.com/a.jpg".to_string(),
                copyright_link: None,
            },
            WallpaperEntry {
                date: NaiveDate::from_ymd_opt(2025, 4, 10).unwrap(),
                headline: Some("B".to_string()),
                title: "B".to_string(),
                url: "https://example.com/b.jpg".to_string(),
                copyright_link: None,
            },
        ]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/a.jpg");
    }

    #[test]
    fn converts_zxyongyo_archive_image_to_entry() {
        let image = ZxyongyoArchiveImage {
            date: "2026-07-04".to_string(),
            title: "紫色花海".to_string(),
            copyright: "瓦朗索勒高原的薰衣草行，普罗旺斯，法国 (© Robert Harding/Shutterstock)"
                .to_string(),
            url_4k: "https://example.com/lavender-4k.jpg".to_string(),
        };

        let entry = zxyongyo_image_to_entry(image).unwrap();
        assert_eq!(entry.date, NaiveDate::from_ymd_opt(2026, 7, 4).unwrap());
        assert_eq!(entry.headline.as_deref(), Some("紫色花海"));
        assert!(entry.title.contains("瓦朗索勒高原"));
        assert_eq!(entry.url, "https://example.com/lavender-4k.jpg");
    }

    #[test]
    fn converts_bing_archive_image_to_entry() {
        let image = BingArchiveImage {
            startdate: "20260713".to_string(),
            enddate: Some("20260714".to_string()),
            urlbase: "/th?id=OHR.NavajoSandstone_ZH-CN5009673011".to_string(),
            copyright: "羚羊峡谷，纳瓦霍族保留地，亚利桑那州，美国 (© Mark Skalny/Getty Images)"
                .to_string(),
            copyrightlink: Some("https://www.bing.com/search?q=羚羊峡谷".to_string()),
            title: "为摇滚而生".to_string(),
        };

        let entry = bing_image_to_entry(image).unwrap();
        assert_eq!(entry.date, NaiveDate::from_ymd_opt(2026, 7, 14).unwrap());
        assert_eq!(entry.headline.as_deref(), Some("为摇滚而生"));
        assert!(entry
            .url
            .contains("OHR.NavajoSandstone_ZH-CN5009673011_UHD.jpg"));
        assert!(entry.url.contains("w=3840&h=2160"));
    }

    #[test]
    fn official_recent_entries_override_local_seed_by_date() {
        let recent = vec![WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
            headline: Some("新标题".to_string()),
            title: "新地点 (© Bing)".to_string(),
            url: "https://example.com/new.jpg".to_string(),
            copyright_link: None,
        }];
        let seed = vec![WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
            headline: None,
            title: "旧地点 (© Cache)".to_string(),
            url: "https://example.com/old.jpg".to_string(),
            copyright_link: None,
        }];

        let entries = merge_entries_prefer_primary(recent, seed);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].headline.as_deref(), Some("新标题"));
        assert_eq!(entries[0].url, "https://example.com/new.jpg");
    }

    #[test]
    fn keeps_archive_date_when_official_api_is_one_day_early() {
        let recent = vec![WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 4).unwrap(),
            headline: Some("紫色花海".to_string()),
            title: "瓦朗索勒高原的薰衣草行，普罗旺斯，法国 (© Robert Harding/Shutterstock)"
                .to_string(),
            url: "https://cn.bing.com/th?id=OHR.LavenderRows_ZH-CN0676810895_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4".to_string(),
            copyright_link: None,
        }];
        let fallback = vec![WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
            headline: Some("紫色花海".to_string()),
            title: "瓦朗索勒高原的薰衣草行，普罗旺斯，法国 (© Robert Harding/Shutterstock)"
                .to_string(),
            url: "https://cn.bing.com/th?id=OHR.LavenderRows_ZH-CN0676810895_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4".to_string(),
            copyright_link: None,
        }];

        let entries = merge_entries_prefer_primary(recent, fallback);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 5).unwrap()
        );
        assert_eq!(entries[0].date_heading(), "2026-07-05 紫色花海");
    }

    #[test]
    fn bundled_archive_backfills_a_date_missing_from_cache() {
        let make_entry = |day, image: &str| WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2020, 4, day).unwrap(),
            headline: None,
            title: format!("2020-04-{day:02}"),
            url: format!("https://cn.bing.com/th?id=OHR.{image}_ZH-CN_UHD.jpg"),
            copyright_link: None,
        };
        let cached = vec![make_entry(3, "April03"), make_entry(5, "April05")];
        let bundled = vec![
            make_entry(3, "April03"),
            make_entry(4, "QingmingCandle2020"),
            make_entry(5, "April05"),
        ];

        let entries = merge_entries_prefer_primary(cached, bundled);
        assert_eq!(entries.len(), 3);
        assert!(entries
            .iter()
            .any(|entry| entry.date == NaiveDate::from_ymd_opt(2020, 4, 4).unwrap()));
    }

    #[test]
    fn rejects_archive_that_drops_historical_coverage() {
        let make_entry = |day| WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 1, day).unwrap(),
            headline: None,
            title: format!("Wallpaper {day}"),
            url: format!("https://example.com/{day}.jpg"),
            copyright_link: None,
        };
        let seed: Vec<_> = (1..=10).map(make_entry).collect();
        let complete: Vec<_> = (1..=10).map(make_entry).collect();
        let truncated: Vec<_> = (5..=10).map(make_entry).collect();

        assert!(archive_covers_seed(&complete, &seed));
        assert!(!archive_covers_seed(&truncated, &seed));
    }
}
