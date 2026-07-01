//! 应用设置：目前只有"自定义下载保存路径"，以 JSON 形式持久化到
//! `%LOCALAPPDATA%\BingWallpaperLib\settings.json`。
//!
//! 设计上刻意保持简单——用户暂时也想不到还需要哪些设置项，所以只暴露一个
//! "下载路径"字段，未来如果需要新增设置（例如开机自启、主题手动覆盖等），
//! 只需在 [`AppSettings`] 结构体中新增字段（并保证 `#[serde(default)]`
//! 以兼容旧版本写入的配置文件）。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 持久化的应用设置。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// 用户手动指定的壁纸下载目录；为 `None` 时使用默认目录
    /// （`%LOCALAPPDATA%\BingWallpaperLib\Wallpapers`）。
    #[serde(default)]
    pub download_dir: Option<PathBuf>,
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
