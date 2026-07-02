use anyhow::{Context, Result};
use std::path::PathBuf;

/// 应用显示名称，用于窗口标题、数据目录等。
pub const APP_NAME: &str = "必应每日壁纸库";

/// 内嵌的 aria2c.exe 二进制数据（构建时打包进主程序，运行时首次启动释放到本地）。
static ARIA2C_BYTES: &[u8] = include_bytes!("../assets/aria2c.exe");

/// 返回应用数据根目录：`%LOCALAPPDATA%\BingWallpaperLib`。
pub fn app_data_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir().context("无法定位 LOCALAPPDATA 目录")?;
    let dir = base.join("BingWallpaperLib");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 默认壁纸下载目录：`%LOCALAPPDATA%\BingWallpaperLib\Wallpapers`。
pub fn default_wallpapers_dir() -> Result<PathBuf> {
    let dir = app_data_dir()?.join("Wallpapers");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 当前生效的壁纸下载目录：若用户在设置中指定了自定义路径则使用该路径，
/// 否则回退到默认目录（见 [`crate::settings::AppSettings::effective_download_dir`]）。
pub fn wallpapers_dir() -> Result<PathBuf> {
    crate::settings::AppSettings::load().effective_download_dir()
}

/// 元数据缓存文件路径（保存解析后的壁纸列表 JSON）。
pub fn cache_file() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("wallpapers_cache.json"))
}

/// 收藏壁纸日期列表路径。
pub fn favorites_file() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("favorites.json"))
}

/// 内部工具目录：`%LOCALAPPDATA%\BingWallpaperLib\bin`，用于释放内嵌的 aria2c.exe。
pub fn tools_dir() -> Result<PathBuf> {
    let dir = app_data_dir()?.join("bin");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 确保 aria2c.exe 已释放到本地工具目录，返回其完整路径。
///
/// aria2c.exe 以 `include_bytes!` 的形式静态嵌入到本程序的可执行文件中，
/// 因此发布时只需分发单个 exe，无需附带任何外部 DLL 或额外文件。
pub fn ensure_aria2c() -> Result<PathBuf> {
    let target = tools_dir()?.join("aria2c.exe");
    let need_write = match std::fs::metadata(&target) {
        Ok(meta) => meta.len() != ARIA2C_BYTES.len() as u64,
        Err(_) => true,
    };
    if need_write {
        std::fs::write(&target, ARIA2C_BYTES).context("释放内置 aria2c.exe 失败")?;
    }
    Ok(target)
}
