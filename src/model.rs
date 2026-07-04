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
    /// 用于本地文件名，形如 `2026-07-01_地牢省立公园_纽芬兰和拉布拉多省_加拿大.jpg`。
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
    const MAX_TITLE_CHARS: usize = 120;

    let prefix = trim_empty_trailing_brackets(copyright_prefix(title).trim());
    let mut result = String::new();
    let mut last_was_separator = false;

    for ch in prefix.chars() {
        let replacement = match ch {
            // Bing 标题通常用中英文逗号分隔“景物、地区、国家”。保留这些地点
            // 信息，但将分隔符统一为 `_`，既方便阅读，也避免文件名中混杂标点。
            ',' | '，' | '、' | ';' | '；' | '|' | '｜' | '·' | '•' => Some('_'),
            // Windows 文件名非法字符，以及少数会让历史数据生成异常文件名的控制字符。
            '<' | '>' | ':' | '"' | '/' | '\\' | '?' | '*' => Some('_'),
            // 破折号常见于“标题 - 地点 - 国家”，归一后和逗号分隔的文件名风格一致。
            '-' | '–' | '—' => Some('_'),
            // 只包裹版权信息的括号会在 copyright_prefix 之后被清掉；标题里的括号
            // 本身没有必要进入文件名，避免生成结尾残留的半个括号。
            '(' | ')' | '（' | '）' | '[' | ']' | '【' | '】' => Some('_'),
            c if c.is_control() => Some('_'),
            c => Some(c),
        };

        let Some(safe) = replacement else {
            continue;
        };

        if safe == '_' {
            while result.ends_with(' ') {
                result.pop();
            }
            if !result.is_empty() && !last_was_separator {
                result.push('_');
                last_was_separator = true;
            }
        } else if safe.is_whitespace() {
            if !result.ends_with(' ') && !last_was_separator {
                result.push(' ');
            }
        } else {
            result.push(safe);
            last_was_separator = false;
        }

        if result.chars().count() >= MAX_TITLE_CHARS {
            break;
        }
    }

    trim_filename_separators(&result).to_string()
}

fn copyright_prefix(title: &str) -> &str {
    ["(©", "（©", "( ©", "（ ©", "©"]
        .iter()
        .filter_map(|marker| title.find(marker))
        .min()
        .map(|index| &title[..index])
        .unwrap_or(title)
}

fn trim_empty_trailing_brackets(value: &str) -> &str {
    let mut value = value.trim();
    loop {
        let trimmed = value.trim_end();
        let Some((open, close)) = trailing_bracket_pair(trimmed) else {
            return trimmed;
        };

        let before_close = &trimmed[..trimmed.len() - close.len_utf8()];
        let Some(open_index) = before_close.rfind(open) else {
            return trimmed;
        };

        if before_close[open_index + open.len_utf8()..]
            .trim()
            .is_empty()
        {
            value = &before_close[..open_index];
        } else {
            return trimmed;
        }
    }
}

fn trailing_bracket_pair(value: &str) -> Option<(char, char)> {
    match value.chars().next_back()? {
        ')' => Some(('(', ')')),
        '）' => Some(('（', '）')),
        ']' => Some(('[', ']')),
        '】' => Some(('【', '】')),
        _ => None,
    }
}

fn trim_filename_separators(value: &str) -> &str {
    value.trim().trim_matches(|ch: char| {
        matches!(
            ch,
            '.' | ' '
                | '_'
                | '-'
                | '–'
                | '—'
                | ','
                | '，'
                | '、'
                | ';'
                | '；'
                | '|'
                | '｜'
                | '('
                | ')'
                | '（'
                | '）'
                | '['
                | ']'
                | '【'
                | '】'
        )
    })
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
    fn file_name_keeps_title_and_location_before_copyright() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            title: "埃斯纳神庙穹顶天花板, 埃及 (© Nick Brundle/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(
            entry.file_name(),
            "2026-07-02_埃斯纳神庙穹顶天花板_埃及.jpg"
        );
    }

    #[test]
    fn file_name_keeps_multiple_chinese_location_parts() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
            title: "小溪上方的萤火虫，冈山县，日本 (© tdub303/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(
            entry.file_name(),
            "2026-07-03_小溪上方的萤火虫_冈山县_日本.jpg"
        );
    }

    #[test]
    fn file_name_keeps_multiple_english_location_parts() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            title: "Dungeon Provincial Park, Newfoundland and Labrador, Canada (© Kaitlyn McLachlan/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(
            entry.file_name(),
            "2026-07-01_Dungeon Provincial Park_Newfoundland and Labrador_Canada.jpg"
        );
    }

    #[test]
    fn file_name_sanitizes_windows_invalid_chars() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            title: "A/B:C*D?E, Somewhere".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-02_A_B_C_D_E_Somewhere.jpg");
    }

    #[test]
    fn file_name_handles_full_width_copyright_marker() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 2).unwrap(),
            title: "古城遗迹，某地，某国 （© Example/Getty Images）".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-02_古城遗迹_某地_某国.jpg");
    }

    #[test]
    fn file_name_handles_bare_copyright_marker() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 4).unwrap(),
            title: "海岸灯塔, Maine, USA © Example/Getty Images".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-04_海岸灯塔_Maine_USA.jpg");
    }

    #[test]
    fn file_name_removes_empty_brackets_left_by_copyright_prefix() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
            title: "湖边森林 (© Example/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-05_湖边森林.jpg");
    }

    #[test]
    fn file_name_normalizes_dash_pipe_and_bracket_separators() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
            title: "Sand dunes - Namib-Naukluft Park | Namibia (Africa) (© Example)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(
            entry.file_name(),
            "2026-07-06_Sand dunes_Namib_Naukluft Park_Namibia_Africa.jpg"
        );
    }

    #[test]
    fn file_name_falls_back_to_date_when_title_is_only_credit() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
            title: "(© Example/Getty Images)".to_string(),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(entry.file_name(), "2026-07-07.jpg");
    }

    #[test]
    fn file_name_trims_separators_after_length_limit() {
        let entry = WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, 8).unwrap(),
            title: format!("{}，地点", "山".repeat(120)),
            url: "https://example.com/a.jpg".to_string(),
        };
        assert_eq!(
            entry.file_name(),
            format!("2026-07-08_{}.jpg", "山".repeat(120))
        );
    }
}
