//! 通过 Win32 API 将指定图片设置为桌面壁纸。

use anyhow::{bail, Context, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDWININICHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

fn to_wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

/// 将本地图片文件设置为当前用户的桌面壁纸（居中/拉伸方式由系统当前设置决定）。
pub fn set_wallpaper(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("壁纸文件不存在: {}", path.display());
    }
    let absolute = std::fs::canonicalize(path).context("无法解析壁纸文件的绝对路径")?;
    // canonicalize 在 Windows 上会产生 `\\?\` 前缀路径，SystemParametersInfoW 不支持，需要去掉。
    let absolute_str = absolute.to_string_lossy();
    let cleaned = absolute_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&absolute_str)
        .to_string();
    let wide = to_wide(std::ffi::OsStr::new(&cleaned));

    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_ptr() as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDWININICHANGE,
        )
    };
    ok.context("调用 SystemParametersInfoW 设置壁纸失败")
}
