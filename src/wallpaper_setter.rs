//! 通过 Windows API 将指定图片设置为桌面壁纸。

use anyhow::{bail, Context, Result};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{Error, HRESULT, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDWININICHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106u32 as i32);

/// 一个可由 Windows 桌面壁纸 COM 接口单独设置的显示器。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonitorInfo {
    /// `IDesktopWallpaper` 使用的显示器设备路径，设置单屏壁纸时必须原样传回。
    pub id: String,
    /// 面向用户展示的短名称。
    pub label: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

fn to_wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn normalized_wallpaper_path(path: &Path) -> Result<String> {
    if !path.exists() {
        bail!("壁纸文件不存在: {}", path.display());
    }

    let absolute = std::fs::canonicalize(path).context("无法解析壁纸文件的绝对路径")?;
    // canonicalize 在 Windows 上会产生 `\\?\` 前缀路径，部分系统壁纸 API 不支持，需要去掉。
    let absolute_str = absolute.to_string_lossy();
    Ok(absolute_str
        .strip_prefix(r"\\?\")
        .unwrap_or(&absolute_str)
        .to_string())
}

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init(context: &'static str) -> Result<Self> {
        // GPUI/Windows 平台层可能已经初始化过 COM；若初始化模式不同，继续使用当前线程已有的 COM 模式。
        let init_hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if init_hr.is_ok() {
            Ok(Self { initialized: true })
        } else if init_hr == RPC_E_CHANGED_MODE {
            Ok(Self { initialized: false })
        } else {
            Err(Error::from(init_hr)).context(context)
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

fn desktop_wallpaper() -> Result<(ComApartment, IDesktopWallpaper)> {
    let apartment = ComApartment::init("初始化桌面壁纸 COM 接口失败")?;
    let wallpaper = unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }
        .context("创建桌面壁纸 COM 接口失败")?;
    Ok((apartment, wallpaper))
}

/// 枚举当前 Windows 识别到的显示器。返回空列表通常表示系统/权限不支持 `IDesktopWallpaper`。
pub fn list_monitors() -> Result<Vec<MonitorInfo>> {
    let (_apartment, wallpaper) = desktop_wallpaper()?;
    let count = unsafe { wallpaper.GetMonitorDevicePathCount() }.context("读取显示器数量失败")?;
    let mut monitors = Vec::with_capacity(count as usize);

    for index in 0..count {
        let monitor_id = unsafe { wallpaper.GetMonitorDevicePathAt(index) }
            .with_context(|| format!("读取显示器 {} 标识失败", index + 1))?;
        let id = unsafe { monitor_id.to_string() }
            .with_context(|| format!("显示器 {} 标识不是合法字符串", index + 1))?;
        unsafe { CoTaskMemFree(Some(monitor_id.as_ptr().cast())) };

        let id_wide = to_wide(std::ffi::OsStr::new(&id));
        let rect = unsafe { wallpaper.GetMonitorRECT(PCWSTR(id_wide.as_ptr())) }
            .with_context(|| format!("读取显示器 {} 尺寸失败", index + 1))?;
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        monitors.push(MonitorInfo {
            id,
            label: format!("显示器 {}（{}×{}）", index + 1, width, height),
            left: rect.left,
            top: rect.top,
            width,
            height,
        });
    }

    Ok(monitors)
}

fn set_wallpaper_legacy(path: &Path) -> Result<()> {
    let cleaned = normalized_wallpaper_path(path)?;
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

/// 将本地图片文件同步设置到全部显示器。
pub fn set_wallpaper_for_all_monitors(path: &Path) -> Result<()> {
    let cleaned = normalized_wallpaper_path(path)?;
    let wide = to_wide(std::ffi::OsStr::new(&cleaned));

    match desktop_wallpaper().and_then(|(_apartment, wallpaper)| unsafe {
        wallpaper
            .SetWallpaper(PCWSTR::null(), PCWSTR(wide.as_ptr()))
            .context("调用 IDesktopWallpaper 设置全部显示器壁纸失败")
    }) {
        Ok(()) => Ok(()),
        Err(err) => {
            log::warn!("IDesktopWallpaper 设置全部显示器失败，回退到 SystemParametersInfoW: {err}");
            set_wallpaper_legacy(path)
        }
    }
}

/// 将本地图片文件设置到指定显示器，其他显示器保持原样。
pub fn set_wallpaper_for_monitor(path: &Path, monitor_id: &str) -> Result<()> {
    if monitor_id.trim().is_empty() {
        bail!("显示器标识为空");
    }

    let cleaned = normalized_wallpaper_path(path)?;
    let wallpaper_wide = to_wide(std::ffi::OsStr::new(&cleaned));
    let monitor_wide = to_wide(std::ffi::OsStr::new(monitor_id));
    let (_apartment, wallpaper) = desktop_wallpaper()?;

    unsafe {
        wallpaper
            .SetWallpaper(
                PCWSTR(monitor_wide.as_ptr()),
                PCWSTR(wallpaper_wide.as_ptr()),
            )
            .context("调用 IDesktopWallpaper 设置指定显示器壁纸失败")
    }
}
