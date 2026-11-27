use anyhow::{Context, Result};
use std::path::PathBuf;

fn directory(base: Option<PathBuf>) -> Result<PathBuf> {
    // Development runners can exercise migration on a disposable copy of a profile.
    #[cfg(debug_assertions)]
    let base = std::env::var_os("BING_WALLPAPER_TEST_PROFILE").map(PathBuf::from).or(base);
    let path = base
        .context("无法定位用户数据目录")?
        .join("BingWallpaperLib");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn app_data_dir() -> Result<PathBuf> {
    directory(dirs::data_local_dir())
}

pub fn default_wallpapers_dir() -> Result<PathBuf> {
    let path = app_data_dir()?.join("Wallpapers");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn cache_file() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        app_data_dir()?
    } else {
        directory(dirs::cache_dir())?
    };
    Ok(base.join("wallpapers_cache.json"))
}

pub fn favorites_file() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("favorites.json"))
}

pub fn downloaded_thumbnails_dir() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        app_data_dir()?
    } else {
        directory(dirs::cache_dir())?
    };
    let path = base.join("downloaded-thumbnails");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn config_dir() -> Result<PathBuf> {
    directory(
        dirs::config_dir()
            .filter(|_| cfg!(target_os = "linux"))
            .or_else(dirs::data_local_dir),
    )
}

pub fn write_atomic(path: &std::path::Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("缺少父目录")?)?;
    temp.write_all(data)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(())
}
