//! 抓取并解析 niumoo/bing-wallpaper 仓库中的 `bing-wallpaper.md`，
//! 得到完整的历史必应每日壁纸列表，并支持增量检测"是否有新的一天"。

use crate::model::WallpaperEntry;
use crate::paths::cache_file;
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use futures::AsyncReadExt;
use http_client::{HttpClient, HttpRequestExt, Request};
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;

/// 原始 Markdown 文件地址（GitHub raw，包含从 2021-02-01 至今的全部记录）。
const SOURCE_URL: &str =
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/main/bing-wallpaper.md";

/// 解析一行形如：
/// `2026-07-02 | [Dungeon Provincial Park... (© xxx)](https://cn.bing.com/th?id=OHR.xxx_UHD.jpg&...)`
/// 的记录。标题中可能包含方括号/圆括号，因此优先匹配以 `](http` 开头的图片链接。
fn line_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\d{4}-\d{2}-\d{2})\s*\|\s*\[(.+)\]\((https?://\S+?)\)\s*$").unwrap()
    })
}

/// 解析整份 markdown 文本为壁纸条目列表（未去重，按文件原始顺序，即最新在前）。
pub fn parse_markdown(text: &str) -> Vec<WallpaperEntry> {
    let re = line_regex();
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with(char::is_numeric) {
            continue;
        }
        let Some(caps) = re.captures(line) else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d") else {
            continue;
        };
        let title = caps[2].trim().to_string();
        let url = caps[3].trim().to_string();
        entries.push(WallpaperEntry { date, title, url });
    }
    entries
}

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

/// 从远端拉取并解析全部壁纸历史（已去重，按日期倒序，即最新的在前）。
pub async fn fetch_all(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let request = Request::get(SOURCE_URL)
        .follow_redirects(http_client::RedirectPolicy::FollowAll)
        .body(Default::default())
        .context("构建请求失败")?;
    let mut response = http
        .send(request)
        .await
        .context("请求 bing-wallpaper.md 失败")?;

    if !response.status().is_success() {
        bail!("bing-wallpaper.md 返回了错误状态码: {}", response.status());
    }

    let mut body = Vec::new();
    response
        .body_mut()
        .read_to_end(&mut body)
        .await
        .context("读取响应内容失败")?;
    let text = String::from_utf8(body).context("响应内容不是合法的 UTF-8 文本")?;

    let mut entries = dedup_by_date(parse_markdown(&text));
    entries.sort_by_key(|e| std::cmp::Reverse(e.date));
    Ok(entries)
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
    fn parses_modern_line_with_query_suffix() {
        let line = "2026-07-02 | [Dungeon Provincial Park, Newfoundland and Labrador, Canada (© Kaitlyn McLachlan/Getty Images)](https://cn.bing.com/th?id=OHR.DungeonPark_EN-US2499621341_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4) ";
        let entries = parse_markdown(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()
        );
        assert!(entries[0].url.starts_with("https://cn.bing.com/th?id=OHR.DungeonPark"));
    }

    #[test]
    fn parses_legacy_line_without_query_suffix() {
        let line = "2023-02-09 | [Ureddplassen, a rest area on the Helgelandskysten scenic route, Norway (© Eyesite/Alamy)](https://cn.bing.com/th?id=OHR.NorwayRestArea_EN-US3474268008_UHD.jpg) ";
        let entries = parse_markdown(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://cn.bing.com/th?id=OHR.NorwayRestArea_EN-US3474268008_UHD.jpg");
    }

    #[test]
    fn dedups_duplicate_dates_keeping_first() {
        let text = "2025-04-10 | [A](https://example.com/a.jpg)\n2025-04-10 | [B](https://example.com/b.jpg)\n";
        let entries = dedup_by_date(parse_markdown(text));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/a.jpg");
    }
}
