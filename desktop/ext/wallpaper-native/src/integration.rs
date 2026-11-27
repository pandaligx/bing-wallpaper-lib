use crate::{paths, region, settings::AppSettings};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
#[cfg(not(windows))]
use std::process::Command;

#[cfg(windows)]
#[path = "../../../../src/folder_picker.rs"]
mod folder_picker;
#[cfg(windows)]
#[path = "../../../../src/startup.rs"]
mod startup;

#[cfg(windows)]
pub(crate) fn has_startup_entry() -> bool {
    startup::is_enabled()
}

pub fn choose_folder() -> Result<String> {
    #[cfg(windows)]
    let path = folder_picker::pick_folder()?;
    #[cfg(target_os = "macos")]
    let path = picked(Command::new("/usr/bin/osascript").args([
        "-e",
        "try",
        "-e",
        "POSIX path of (choose folder)",
        "-e",
        "on error number -128",
        "-e",
        "return \"\"",
        "-e",
        "end try",
    ]))?;
    #[cfg(target_os = "linux")]
    let path = {
        let kde = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("kde");
        let (first, second) = if kde {
            ("kdialog", "zenity")
        } else {
            ("zenity", "kdialog")
        };
        let pick = |tool: &str| {
            let mut cmd = Command::new(tool);
            if tool == "kdialog" {
                cmd.arg("--getexistingdirectory");
            } else {
                cmd.args(["--file-selection", "--directory"]);
            }
            picked(&mut cmd)
        };
        pick(first)
            .or_else(|_| pick(second))
            .context("请输入文件夹的绝对路径")?
    };
    Ok(path.map_or_else(String::new, |p| p.to_string_lossy().into_owned()))
}

#[cfg(not(windows))]
fn picked(command: &mut Command) -> Result<Option<PathBuf>> {
    let result = command.output()?;
    if result.status.code() == Some(1) {
        return Ok(None);
    }
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value = String::from_utf8(result.stdout)?;
    Ok((!value.trim().is_empty()).then(|| PathBuf::from(value.trim_end())))
}

pub fn open_location(kind: &str) -> Result<()> {
    let location = if kind == "folder" {
        AppSettings::load().effective_download_dir()?
    } else {
        let repo = match region::selected()?.context("请选择更新地区")? {
            region::Source::GitHub => "https://github.com/pandaligx/bing-wallpaper-lib",
            region::Source::GitCode => "https://gitcode.com/gcw_qWIjmve7/bing-wallpaper-lib",
        };
        PathBuf::from(match kind {
            "project" => repo.to_owned(),
            "releases" => format!("{repo}/releases"),
            _ => anyhow::bail!("无效的位置"),
        })
    };
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::{PCWSTR, w},
        };
        let wide: Vec<u16> = location.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: only a configured directory or fixed project URL is passed in a live UTF-16 buffer.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        ensure!(result.0 as usize > 32, "无法打开位置");
    }
    #[cfg(not(windows))]
    {
        let tool = if cfg!(target_os = "macos") {
            "/usr/bin/open"
        } else {
            "xdg-open"
        };
        let output = Command::new(tool).arg(location).output()?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub fn tray_available() -> bool {
    #[cfg(target_os = "linux")]
    return Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.kde.StatusNotifierWatcher",
            "--object-path",
            "/StatusNotifierWatcher",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.kde.StatusNotifierWatcher",
            "IsStatusNotifierHostRegistered",
        ])
        .output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("true"));
    #[cfg(not(target_os = "linux"))]
    true
}

pub fn startup_enabled(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        startup::set_enabled(enabled)?;
        ensure!(startup::is_enabled() == enabled, "无法验证登录启动设置");
    }
    #[cfg(target_os = "linux")]
    {
        let dir = dirs::config_dir()
            .context("找不到 XDG 配置目录")?
            .join("autostart");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("cn.lgxng.bingwallpaperlib.desktop");
        if enabled {
            let exe =
                PathBuf::from(std::env::var_os("APPIMAGE").context("请从 AppImage 设置登录启动")?)
                    .canonicalize()?;
            let exe = exe.to_str().context("无效的应用路径")?;
            ensure!(!exe.contains(['\n', '\r']), "无效的应用路径");
            let mut escaped = String::new();
            for c in exe.chars() {
                match c {
                    '%' => escaped.push('%'),
                    '\\' => escaped.push_str(r"\\\"),
                    '"' | '$' | '\u{60}' => escaped.push_str(r"\\"),
                    _ => (),
                }
                escaped.push(c);
            }
            paths::write_atomic(&file,format!("[Desktop Entry]\nType=Application\nName=Bing Wallpaper Library\nExec=\"{escaped}\" --background\n").as_bytes())?;
        } else if file.try_exists()? {
            std::fs::remove_file(file)?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = enabled;
        anyhow::bail!("请使用系统登录项设置");
    }
    #[cfg(not(target_os = "macos"))]
    Ok(())
}

pub fn clear_cache() -> Result<()> {
    crate::local_thumbnails::clear()?;
    let file = paths::cache_file()?;
    if file.try_exists()? {
        std::fs::remove_file(file)?;
    }
    Ok(())
}
