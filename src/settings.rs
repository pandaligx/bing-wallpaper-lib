//! 应用设置，以 JSON 形式持久化到 `%LOCALAPPDATA%\BingWallpaperLib\settings.json`。
//!
//! 每个新增字段都应使用 `#[serde(default)]`，以兼容旧版本写入的配置文件。

use anyhow::{Context, Result};
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

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "跟随系统",
            Self::Light => "白天模式",
            Self::Dark => "夜间模式",
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

/// 持久化的应用设置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// 用户手动指定的壁纸下载目录；为 `None` 时使用默认目录
    /// （`%LOCALAPPDATA%\BingWallpaperLib\Wallpapers`）。
    #[serde(default)]
    pub download_dir: Option<PathBuf>,
    /// 主题偏好；默认跟随 Windows 系统深色/浅色模式。
    #[serde(default)]
    pub theme_preference: ThemePreference,
    /// 设置桌面壁纸时的目标显示器；默认同步全部显示器。
    #[serde(default)]
    pub wallpaper_target: WallpaperTarget,
}

impl AppSettings {
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
