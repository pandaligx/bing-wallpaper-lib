//! 应用设置，以 JSON 形式持久化到 `%LOCALAPPDATA%\BingWallpaperLib\settings.json`。
//!
//! 每个新增字段都应使用 `#[serde(default)]`，以兼容旧版本写入的配置文件。

use crate::i18n::LanguagePreference;
use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 主题偏好：默认跟随系统，也允许用户手动固定为白天/夜间。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub fn from_settings() -> Self {
        AppSettings::load().theme_preference
    }

    pub fn label(self, language: LanguagePreference) -> &'static str {
        match self {
            Self::System => language.t("System"),
            Self::Light => language.t("Light"),
            Self::Dark => language.t("Dark"),
        }
    }
}

/// “设为桌面壁纸”按钮的目标：同步所有显示器，或只设置某一个显示器。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "monitor_id")]
pub enum WallpaperTarget {
    #[default]
    All,
    Monitor(String),
}

impl WallpaperTarget {
    pub fn monitor_id(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::Monitor(id) => Some(id),
        }
    }
}

/// 下载/设置桌面壁纸时使用的全局图片分辨率。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadResolution {
    Original,
    #[default]
    FourK,
    TwoK,
    OneK,
}

impl DownloadResolution {
    pub const ALL: [Self; 4] = [Self::Original, Self::FourK, Self::TwoK, Self::OneK];

    pub fn label(self, language: LanguagePreference) -> &'static str {
        match self {
            Self::Original => language.t("Original"),
            Self::FourK => "4K",
            Self::TwoK => "2K",
            Self::OneK => "1K",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Original => "UHD",
            Self::FourK => "3840×2160",
            Self::TwoK => "2560×1440",
            Self::OneK => "1920×1080",
        }
    }

    pub fn status_label(self, language: LanguagePreference) -> &'static str {
        match self {
            Self::Original => language.t("Original"),
            Self::FourK => "4K-3840×2160",
            Self::TwoK => "2K-2560×1440",
            Self::OneK => "1K-1920×1080",
        }
    }
}

/// 每日自动壁纸来源。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoWallpaperSource {
    /// 每天自动下载最新 Bing 壁纸并设为桌面壁纸。
    #[default]
    Latest,
    /// 从全部历史壁纸中随机选一张。
    RandomAll,
    /// 从“我的收藏”中随机选一张。
    RandomFavorites,
}

impl AutoWallpaperSource {
    pub fn label(self, language: LanguagePreference) -> &'static str {
        match self {
            Self::Latest => language.t("Latest daily wallpaper"),
            Self::RandomAll => language.t("Random from all history"),
            Self::RandomFavorites => language.t("Random from favorites"),
        }
    }
}

/// Windows 任务计划周期执行时，在“当天首张最新壁纸”之后使用的随机来源。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodicWallpaperSource {
    /// 从全部历史壁纸中随机选择。
    #[default]
    RandomAll,
    /// 从收藏中随机选择；收藏为空时回退到全部历史。
    RandomFavorites,
}

impl PeriodicWallpaperSource {
    pub fn label(self, language: LanguagePreference) -> &'static str {
        match self {
            Self::RandomAll => language.t("Random from all history"),
            Self::RandomFavorites => language.t("Random from favorites"),
        }
    }
}

fn default_auto_wallpaper_hour() -> u8 {
    8
}

fn default_periodic_interval_minutes() -> u16 {
    60
}

fn default_periodic_daily_first_latest() -> bool {
    true
}

fn default_background_resident_enabled() -> bool {
    true
}

/// 持久化的应用设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 用户手动指定的壁纸下载目录；为 `None` 时使用默认目录
    /// （`%LOCALAPPDATA%\BingWallpaperLib\Wallpapers`）。
    #[serde(default)]
    pub download_dir: Option<PathBuf>,
    /// 主题偏好；默认跟随 Windows 系统深色/浅色模式。
    #[serde(default)]
    pub theme_preference: ThemePreference,
    /// 界面语言；默认跟随 Windows，无法识别时回退到英文。
    #[serde(default)]
    pub language: LanguagePreference,
    /// 设置桌面壁纸时的目标显示器；默认同步全部显示器。
    #[serde(default)]
    pub wallpaper_target: WallpaperTarget,
    #[serde(default)]
    pub download_resolution: DownloadResolution,
    /// 是否创建系统托盘图标并保持后台能力；默认关闭。
    #[serde(default = "default_background_resident_enabled")]
    pub background_resident_enabled: bool,
    /// 是否注册 Windows 开机自启；默认关闭。
    #[serde(default)]
    pub startup_enabled: bool,
    /// 是否启用每日自动更换壁纸。
    #[serde(default)]
    pub auto_wallpaper_enabled: bool,
    /// 每日自动壁纸来源。
    #[serde(default)]
    pub auto_wallpaper_source: AutoWallpaperSource,
    /// 每日自动执行小时（0~23），默认 8 点。
    #[serde(default = "default_auto_wallpaper_hour")]
    pub auto_wallpaper_hour: u8,
    /// 每日自动执行分钟（0~59），默认 0 分。
    #[serde(default)]
    pub auto_wallpaper_minute: u8,
    /// 每日自动壁纸成功设置后是否自动退出程序。
    #[serde(default)]
    pub auto_wallpaper_exit_after_done: bool,
    /// 上次自动执行日期，用于避免同一天重复执行。
    #[serde(default)]
    pub last_auto_wallpaper_date: Option<NaiveDate>,
    /// 是否已注册“登录 + 周期重复”的 Windows 壁纸任务计划。
    #[serde(default)]
    pub periodic_task_enabled: bool,
    /// 周期任务的重复间隔（总分钟数），有效范围 1~1439。
    #[serde(default = "default_periodic_interval_minutes")]
    pub periodic_interval_minutes: u16,
    /// 每个自然日首次执行周期任务时是否优先使用 Bing 最新壁纸。
    #[serde(default = "default_periodic_daily_first_latest")]
    pub periodic_daily_first_latest: bool,
    /// 当天首次执行之后的壁纸随机来源。
    #[serde(default)]
    pub periodic_wallpaper_source: PeriodicWallpaperSource,
    /// 上次成功使用“当天首张最新壁纸”的日期。
    #[serde(default)]
    pub last_periodic_latest_date: Option<NaiveDate>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            download_dir: None,
            theme_preference: ThemePreference::default(),
            language: LanguagePreference::default(),
            wallpaper_target: WallpaperTarget::default(),
            download_resolution: DownloadResolution::default(),
            background_resident_enabled: true,
            startup_enabled: false,
            auto_wallpaper_enabled: false,
            auto_wallpaper_source: AutoWallpaperSource::default(),
            auto_wallpaper_hour: default_auto_wallpaper_hour(),
            auto_wallpaper_minute: 0,
            auto_wallpaper_exit_after_done: false,
            last_auto_wallpaper_date: None,
            periodic_task_enabled: false,
            periodic_interval_minutes: default_periodic_interval_minutes(),
            periodic_daily_first_latest: true,
            periodic_wallpaper_source: PeriodicWallpaperSource::default(),
            last_periodic_latest_date: None,
        }
    }
}

impl AppSettings {
    pub const MIN_PERIODIC_INTERVAL_MINUTES: u16 = 1;
    pub const MAX_PERIODIC_INTERVAL_MINUTES: u16 = 23 * 60 + 59;

    pub fn normalized_periodic_interval_minutes(&self) -> u16 {
        self.periodic_interval_minutes.clamp(
            Self::MIN_PERIODIC_INTERVAL_MINUTES,
            Self::MAX_PERIODIC_INTERVAL_MINUTES,
        )
    }

    fn file_path() -> Result<PathBuf> {
        Ok(crate::paths::app_data_dir()?.join("settings.json"))
    }

    /// 从磁盘加载设置；文件不存在或解析失败时返回默认值（不视为错误）。
    pub fn load() -> Self {
        Self::try_load().unwrap_or_default()
    }

    fn try_load() -> Result<Self> {
        let path = Self::file_path()?;
        let data = std::fs::read(&path).context("读取设置文件失败")?;
        let settings: Self = serde_json::from_slice(&data).context("解析设置文件失败")?;
        Ok(settings)
    }

    /// 将当前设置写回磁盘。
    pub fn save(&self) -> Result<()> {
        let path = Self::file_path()?;
        let data = serde_json::to_vec_pretty(self).context("序列化设置失败")?;
        std::fs::write(&path, data).context("写入设置文件失败")?;
        Ok(())
    }

    /// 有效的壁纸下载目录：优先使用用户自定义路径（若能创建/写入），
    /// 否则回退到默认目录。
    pub fn effective_download_dir(&self) -> Result<PathBuf> {
        if let Some(dir) = &self.download_dir {
            std::fs::create_dir_all(dir).context("创建自定义下载目录失败")?;
            Ok(dir.clone())
        } else {
            crate::paths::default_wallpapers_dir()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_without_language_follow_the_system() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.language, LanguagePreference::System);
    }

    #[test]
    fn language_preference_round_trips_through_json() {
        let settings = AppSettings {
            language: LanguagePreference::French,
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.language, LanguagePreference::French);
    }

    #[test]
    fn old_settings_get_safe_periodic_task_defaults() {
        let settings: AppSettings = serde_json::from_str("{}").unwrap();
        assert!(!settings.periodic_task_enabled);
        assert_eq!(settings.periodic_interval_minutes, 60);
        assert!(settings.periodic_daily_first_latest);
        assert_eq!(
            settings.periodic_wallpaper_source,
            PeriodicWallpaperSource::RandomAll
        );
        assert_eq!(settings.last_periodic_latest_date, None);
    }

    #[test]
    fn periodic_interval_is_clamped_to_supported_range() {
        let mut settings = AppSettings {
            periodic_interval_minutes: 0,
            ..Default::default()
        };
        assert_eq!(settings.normalized_periodic_interval_minutes(), 1);

        settings.periodic_interval_minutes = u16::MAX;
        assert_eq!(
            settings.normalized_periodic_interval_minutes(),
            23 * 60 + 59
        );
    }
}
