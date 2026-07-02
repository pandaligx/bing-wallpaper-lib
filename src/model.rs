use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 单条必应每日壁纸记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WallpaperEntry {
    /// 壁纸对应日期（必应发布日期）。
    pub date: NaiveDate,
    /// 壁纸标题/描述（包含摄影师/来源信息）。
    pub title: String,
    /// 原始高清图片地址（3840x2160）。
    pub url: String,
}

impl WallpaperEntry {
    /// 用于本地文件名，形如 `2026-07-01_地牢省立公园.jpg`。
    pub fn file_name(&self) -> String {
        let title = filename_title_part(&self.title);
        if title.is_empty() {
            format!("{}.jpg", self.date.format("%Y-%m-%d"))
        } else {
            format!("{}_{}.jpg", self.date.format("%Y-%m-%d"), title)
        }
    }

    /// 用于列表/网格展示的缩略图地址。
    ///
    /// 现代记录（2023-02-09 起）的 `url` 带有 `w=3840&h=2160` 查询参数，
    /// 替换为更小的尺寸可以大幅减少缩略图加载的流量与解码开销；
    /// 更早期的记录没有这个查询参数（裸 `.jpg` 链接），无法改变分辨率，
    /// 此时原样返回完整地址。下载/设为壁纸时应始终使用 [`WallpaperEntry::url`]
    /// 原始高清地址，而不是这里的缩略图地址。
    pub fn thumbnail_url(&self) -> String {
        const FULL_SIZE: &str = "w=3840&h=2160";
        const THUMB_SIZE: &str = "w=320&h=180";
        if self.url.contains(FULL_SIZE) {
            self.url.replace(FULL_SIZE, THUMB_SIZE)
        } else {
            self.url.clone()
        }
    }
}

fn filename_title_part(title: &str) -> String {
    let prefix = title
        .split([',', '，'])
        .next()
        .unwrap_or(title)
        .split("(©")
        .next()
        .unwrap_or(title)
        .trim();

    let mut result = String::new();
    for ch in prefix.chars() {
        let safe = match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        };
        if safe.is_whitespace() {
            result.push(' ');
        } else {
            result.push(safe);
        }
        if result.chars().count() >= 60 {
            break;
        }
    }
    result.trim().trim_matches('.').to_string()
}

/// 按 "年-月" 分组后的壁纸列表，用于左侧导航栏。
#[derive(Debug, Clone)]
pub struct MonthGroup {
    /// 分组键，形如 `2026-07`。
    pub key: String,
    /// 年份。
    pub year: i32,
    /// 月份 (1-12)。
    pub month: u32,
    /// 该月下的全部壁纸条目，按日期倒序排列。
    pub entries: Vec<WallpaperEntry>,
}

/// 将壁纸条目按年月分组，结果按时间倒序（最新月份在前）。
pub fn group_by_month(entries: &[WallpaperEntry]) -> Vec<MonthGroup> {
    let mut map: BTreeMap<(i32, u32), Vec<WallpaperEntry>> = BTreeMap::new();
    for entry in entries {
        use chrono::Datelike;
        let key = (entry.date.year(), entry.date.month());
        map.entry(key).or_default().push(entry.clone());
    }

    let mut groups: Vec<MonthGroup> = map
        .into_iter()
        .map(|((year, month), mut entries)| {
            entries.sort_by_key(|e| std::cmp::Reverse(e.date));
            MonthGroup {
                key: format!("{year:04}-{month:02}"),
                year,
                month,
                entries,
            }
        })
        .collect();

    groups.sort_by_key(|g| std::cmp::Reverse((g.year, g.month)));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_uses_date_and_title_before_location() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            title: "埃斯纳神庙穹顶天花板, 埃及 (© Nick Brundle/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-02_埃斯纳神庙穹顶天花板.jpg");
    }

    #[test]
    fn file_name_sanitizes_windows_invalid_chars() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            title: "A/B:C*D?E, Somewhere".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-02_A_B_C_D_E.jpg");
    }
}
