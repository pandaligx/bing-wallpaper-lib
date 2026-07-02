//! 抓取并解析 niumoo/bing-wallpaper 仓库中的壁纸列表。
//!
//! 默认优先使用 `zh-cn/bing-wallpaper.md`（中文标题版本），并用根目录下的英文版
//! `bing-wallpaper.md` 补齐中文版缺失的历史日期，最终得到完整的历史必应每日壁纸列表，
//! 同时支持增量检测"是否有新的一天"。

use crate::model::WallpaperEntry;
use crate::paths::cache_file;
use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use futures::AsyncReadExt;
use http_client::{HttpClient, HttpRequestExt, Request};
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;

/// 候选源地址列表，按顺序依次尝试，第一个成功的即返回。
///
/// 自 v0.2.4 起优先使用中文标题版本 `zh-cn/bing-wallpaper.md`，所有壁纸的标题、地点、
/// 作者说明均尽量以中文展示，更适合中文用户浏览。中文版文件的每条记录之间会多出一个空行，
/// 但这并不影响解析——`parse_markdown` 已经会跳过空行与非日期开头的行。
///
/// 自 v0.2.5 起会额外拉取英文版 `bing-wallpaper.md` 作为补全集：同一日期优先保留中文记录，
/// 只有中文版缺失的日期才使用英文记录，从而避免同一张图在中英文两个源之间重复出现。
///
/// `raw.githubusercontent.com` 在中国大陆部分网络环境下无法直接访问（需要科学上网），
/// 因此两组源地址都优先使用 [jsDelivr](https://www.jsdelivr.com/) CDN 镜像 GitHub 仓库内容，
/// 绝大多数国内网络环境下无需 VPN 即可直接访问（代价是 jsDelivr 对 GitHub 内容有数小时级的
/// 缓存延迟，考虑到本项目本身每 30 分钟才检查一次更新，这点延迟可以接受）；
/// GitHub 官方地址作为兼容科学上网用户以及 jsDelivr 自身出现问题时的备选。
const CHINESE_SOURCE_URLS: &[&str] = &[
    "https://cdn.jsdelivr.net/gh/niumoo/bing-wallpaper@main/zh-cn/bing-wallpaper.md",
    "https://fastly.jsdelivr.net/gh/niumoo/bing-wallpaper@main/zh-cn/bing-wallpaper.md",
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/main/zh-cn/bing-wallpaper.md",
];

const ENGLISH_SOURCE_URLS: &[&str] = &[
    "https://cdn.jsdelivr.net/gh/niumoo/bing-wallpaper@main/bing-wallpaper.md",
    "https://fastly.jsdelivr.net/gh/niumoo/bing-wallpaper@main/bing-wallpaper.md",
    "https://raw.githubusercontent.com/niumoo/bing-wallpaper/main/bing-wallpaper.md",
];

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

/// 合并两份壁纸列表：同一日期优先保留 primary 中的记录，fallback 只补齐缺失日期。
fn merge_entries_prefer_primary(
    primary: Vec<WallpaperEntry>,
    fallback: Vec<WallpaperEntry>,
) -> Vec<WallpaperEntry> {
    let mut entries = dedup_by_date(primary);
    let mut seen_dates: HashSet<NaiveDate> = entries.iter().map(|entry| entry.date).collect();

    for entry in dedup_by_date(fallback) {
        if seen_dates.insert(entry.date) {
            entries.push(entry);
        }
    }

    entries.sort_by_key(|entry| std::cmp::Reverse(entry.date));
    entries
}

/// 从一组候选源依次尝试拉取并解析壁纸历史（已去重，保持源文件原始顺序）。
async fn fetch_entries_from_sources(
    http: Arc<dyn HttpClient>,
    source_name: &str,
    urls: &[&str],
) -> Result<Vec<WallpaperEntry>> {
    let mut last_err = None;
    for &url in urls {
        match fetch_text(http.clone(), url).await {
            Ok(text) => return Ok(dedup_by_date(parse_markdown(&text))),
            Err(err) => {
                log::warn!("从 {source_name}源 {url} 获取壁纸列表失败: {err}");
                last_err = Some(err);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("没有可用的{source_name}壁纸数据源")))
}

/// 从候选源依次尝试拉取并解析全部壁纸历史（已去重，按日期倒序，即最新的在前）。
pub async fn fetch_all(http: Arc<dyn HttpClient>) -> Result<Vec<WallpaperEntry>> {
    let chinese = fetch_entries_from_sources(http.clone(), "中文", CHINESE_SOURCE_URLS).await;
    let english = fetch_entries_from_sources(http, "英文", ENGLISH_SOURCE_URLS).await;

    match (chinese, english) {
        (Ok(chinese), Ok(english)) => Ok(merge_entries_prefer_primary(chinese, english)),
        (Ok(mut chinese), Err(err)) => {
            log::warn!("英文补全集不可用，仅使用中文壁纸列表: {err}");
            chinese.sort_by_key(|entry| std::cmp::Reverse(entry.date));
            Ok(chinese)
        }
        (Err(err), Ok(mut english)) => {
            log::warn!("中文壁纸列表不可用，退回英文壁纸列表: {err}");
            english.sort_by_key(|entry| std::cmp::Reverse(entry.date));
            Ok(english)
        }
        (Err(chinese_err), Err(english_err)) => Err(anyhow::anyhow!(
            "没有可用的壁纸数据源；中文源错误: {chinese_err}; 英文源错误: {english_err}"
        )),
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
        .read_to_end(&mut body)
        .await
        .context("读取响应内容失败")?;
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
    fn parses_modern_line_with_query_suffix() {
        let line = "2026-07-02 | [Dungeon Provincial Park, Newfoundland and Labrador, Canada (© Kaitlyn McLachlan/Getty Images)](https://cn.bing.com/th?id=OHR.DungeonPark_EN-US2499621341_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4) ";
        let entries = parse_markdown(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()
        );
        assert!(entries[0]
            .url
            .starts_with("https://cn.bing.com/th?id=OHR.DungeonPark"));
    }

    #[test]
    fn parses_legacy_line_without_query_suffix() {
        let line = "2023-02-09 | [Ureddplassen, a rest area on the Helgelandskysten scenic route, Norway (© Eyesite/Alamy)](https://cn.bing.com/th?id=OHR.NorwayRestArea_EN-US3474268008_UHD.jpg) ";
        let entries = parse_markdown(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].url,
            "https://cn.bing.com/th?id=OHR.NorwayRestArea_EN-US3474268008_UHD.jpg"
        );
    }

    #[test]
    fn parses_chinese_line_from_zh_cn_source() {
        // 来自 zh-cn/bing-wallpaper.md 的真实格式：中文标题 + _ZH-CN 变体的图片 URL。
        let line = "2026-07-02 | [埃斯纳神庙穹顶天花板, 埃及 (© Nick Brundle Photography/Getty Images)](https://cn.bing.com/th?id=OHR.TempleEsna_ZH-CN9834689523_UHD.jpg&rf=LaDigue_UHD.jpg&pid=hp&w=3840&h=2160&rs=1&c=4)";
        let entries = parse_markdown(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()
        );
        assert!(entries[0].title.contains("埃斯纳神庙"));
        assert!(entries[0].url.contains("_ZH-CN"));
    }

    #[test]
    fn parses_zh_cn_file_with_blank_lines_between_entries() {
        // zh-cn 版本每两条记录之间会多出一个空行，parse_markdown 应能正确跳过。
        let text = "## Bing Wallpaper\n2026-07-02 | [埃斯纳神庙穹顶天花板, 埃及 (© A/B)](https://cn.bing.com/th?id=OHR.TempleEsna_ZH-CN1_UHD.jpg&w=3840&h=2160)\n\n2026-07-01 | [地牢省立公园, 加拿大 (© C/D)](https://cn.bing.com/th?id=OHR.DungeonPark_ZH-CN2_UHD.jpg&w=3840&h=2160)\n";
        let entries = parse_markdown(text);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].title.contains("埃斯纳神庙"));
        assert!(entries[1].title.contains("地牢省立公园"));
    }

    #[test]
    fn merges_chinese_entries_with_english_fallback_by_date() {
        let chinese = parse_markdown(
            "2026-07-02 | [中文标题](https://cn.bing.com/th?id=OHR.Today_ZH-CN_UHD.jpg)\n",
        );
        let english = parse_markdown(
            "2026-07-02 | [English title](https://cn.bing.com/th?id=OHR.Today_EN-US_UHD.jpg)\n2021-02-01 | [Old English title](https://cn.bing.com/th?id=OHR.Old_EN-US_UHD.jpg)\n",
        );

        let entries = merge_entries_prefer_primary(chinese, english);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].date,
            NaiveDate::from_ymd_opt(2026, 7, 2).unwrap()
        );
        assert_eq!(entries[0].title, "中文标题");
        assert!(entries[0].url.contains("_ZH-CN"));
        assert_eq!(
            entries[1].date,
            NaiveDate::from_ymd_opt(2021, 2, 1).unwrap()
        );
        assert_eq!(entries[1].title, "Old English title");
    }

    #[test]
    fn dedups_duplicate_dates_keeping_first() {
        let text = "2025-04-10 | [A](https://example.com/a.jpg)\n2025-04-10 | [B](https://example.com/b.jpg)\n";
        let entries = dedup_by_date(parse_markdown(text));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, "https://example.com/a.jpg");
    }
}
