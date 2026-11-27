use crate::paths;
use anyhow::{Context, Result, ensure};

const GITCODE_ARCHIVE: &str = "https://api.gitcode.com/api/v5/repos/gcw_qWIjmve7/bing-wallpaper-lib/raw/assets/data/zxyongyo-bing-wallpaper.json?ref=main";
const GITHUB_ARCHIVE: &str = "https://raw.githubusercontent.com/pandaligx/bing-wallpaper-lib/main/assets/data/zxyongyo-bing-wallpaper.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    GitCode,
    GitHub,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Self::GitCode => "GitCode",
            Self::GitHub => "GitHub",
        }
    }
    pub fn archive(self) -> &'static str {
        match self {
            Self::GitCode => GITCODE_ARCHIVE,
            Self::GitHub => GITHUB_ARCHIVE,
        }
    }
}

fn source_for_region(region: &str) -> Option<Source> {
    let country = region.trim();
    if country.len() != 2
        || !country.bytes().all(|b| b.is_ascii_alphabetic())
        || country.eq_ignore_ascii_case("ZZ")
    {
        return None;
    }
    Some(if country.eq_ignore_ascii_case("CN") {
        Source::GitCode
    } else {
        Source::GitHub
    })
}

#[cfg(windows)]
fn system_region() -> Option<String> {
    let mut buf = [0u16; 86];
    // SAFETY: the API receives a writable buffer and its actual length.
    let length = unsafe { windows::Win32::Globalization::GetUserDefaultGeoName(&mut buf) };
    (length > 1).then(|| String::from_utf16_lossy(&buf[..length as usize - 1]))
}

#[cfg(not(windows))]
fn system_region() -> Option<String> {
    #[cfg(target_os = "macos")]
    let locale = {
        let cmd = std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            .ok()?;
        if !cmd.status.success() {
            return None;
        }
        String::from_utf8(cmd.stdout).ok()?
    };
    #[cfg(target_os = "linux")]
    let locale = ["LC_ALL", "LC_ADDRESS", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))?;
    locale
        .trim()
        .split(['.', '@'])
        .next()?
        .split(['-', '_'])
        .skip(1)
        .find(|s| s.len() == 2 && s.bytes().all(|b| b.is_ascii_alphabetic()))
        .map(str::to_owned)
}

pub fn selected() -> Result<Option<Source>> {
    let path = paths::config_dir()?.join("update-region");
    match std::fs::read_to_string(path) {
        Ok(s) => match s.trim() {
            "cn" => return Ok(Some(Source::GitCode)),
            "international" => return Ok(Some(Source::GitHub)),
            "auto" => (),
            _ => anyhow::bail!("无法读取已保存的更新地区"),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    Ok(system_region().as_deref().and_then(source_for_region))
}

pub fn select(value: &str) -> Result<()> {
    ensure!(
        matches!(value, "auto" | "cn" | "international"),
        "无效的更新地区"
    );
    let path = paths::config_dir()?.join("update-region");
    paths::write_atomic(&path, value.as_bytes()).context("保存更新地区失败")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn country_selects_source_independently_of_language() {
        assert_eq!(source_for_region("CN"), Some(Source::GitCode));
        for region in ["US", "HK", "TW", "JP"] {
            assert_eq!(source_for_region(region), Some(Source::GitHub));
        }
        for unknown in ["", "zh-CN", "001", "ZZ"] {
            assert_eq!(source_for_region(unknown), None);
        }
    }
}
