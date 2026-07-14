use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::PathBuf;

/// 应用显示名称，用于数据目录、关于弹窗等。
pub const APP_NAME: &str = "必应每日壁纸库";

/// 窗口标题/任务栏标题，带当前软件版本号。
pub fn app_window_title() -> String {
    format!("{} v{}", APP_NAME, env!("CARGO_PKG_VERSION"))
}

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

/// 已下载壁纸画廊的小尺寸缩略图缓存目录。
pub fn downloaded_thumbnails_dir() -> Result<PathBuf> {
    let dir = app_data_dir()?.join("downloaded-thumbnails");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 内部工具目录：`%LOCALAPPDATA%\BingWallpaperLib\bin`，用于释放内嵌的 aria2c.exe。
pub fn tools_dir() -> Result<PathBuf> {
    let dir = app_data_dir()?.join("bin");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 将内嵌 aria2c.exe 释放为本次进程专用的随机文件，返回其完整路径。
///
/// 不复用固定文件名，避免应用以管理员权限启动后执行被普通用户进程预先替换的
/// `%LOCALAPPDATA%` 文件。调用方应在 aria2 子进程退出后删除返回的临时文件。
pub fn extract_aria2c() -> Result<PathBuf> {
    let dir = tools_dir()?;
    for _ in 0..8 {
        let target = dir.join(format!("aria2c-{}.exe", uuid::Uuid::new_v4().simple()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => {
                file.write_all(ARIA2C_BYTES)
                    .context("释放内置 aria2c.exe 失败")?;
                file.flush().context("刷新内置 aria2c.exe 失败")?;
                return Ok(target);
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).context("创建内置 aria2c.exe 临时文件失败"),
        }
    }
    bail!("无法创建唯一的 aria2c.exe 临时文件")
}
