use crate::settings::WallpaperTarget;
use anyhow::{Context, Result};
use std::path::Path;

pub fn available() -> bool {
    #[cfg(target_os = "linux")]
    {
        let desktop = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if desktop.split(':').any(|s| s == "gnome") {
            return run("gsettings", &["list-schemas"])
                .is_ok_and(|s| s.lines().any(|line| line == "org.gnome.desktop.background"));
        }
        if desktop.split(':').any(|s| s == "kde") {
            return run(
                "gdbus",
                &[
                    "call",
                    "--session",
                    "--dest",
                    "org.freedesktop.DBus",
                    "--object-path",
                    "/org/freedesktop/DBus",
                    "--method",
                    "org.freedesktop.DBus.NameHasOwner",
                    "org.kde.plasmashell",
                ],
            )
            .is_ok_and(|s| s.contains("true"));
        }
        run(
            "gdbus",
            &[
                "introspect",
                "--session",
                "--dest",
                "org.freedesktop.portal.Desktop",
                "--object-path",
                "/org/freedesktop/portal/desktop",
                "--xml",
            ],
        )
        .is_ok_and(|s| s.contains("name=\"org.freedesktop.portal.Wallpaper\""))
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[cfg(windows)]
#[path = "../../../../src/wallpaper_setter.rs"]
pub mod windows;

pub fn monitors() -> Result<String> {
    #[cfg(windows)]
    {
        let list: Vec<_> = windows::list_monitors()?.into_iter().map(|m|
            serde_json::json!({"id":m.id,"label":m.label,"width":m.width,"height":m.height,"left":m.left,"top":m.top})
        ).collect();
        Ok(serde_json::to_string(&list)?)
    }
    #[cfg(target_os = "linux")]
    {
        if std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .split(':')
            .any(|v| v == "kde")
        {
            let reply = plasma(
                "print('NSH[' + desktopsForActivity(currentActivity()).filter(d => d.screen >= 0).map(d => d.id + ':' + d.screen).join(',') + ']')",
            )?;
            let body = reply
                .split_once("NSH[")
                .and_then(|(_, s)| s.split_once(']'))
                .context("Plasma 未返回活动屏幕")?
                .0;
            let mut list = Vec::new();
            for pair in body.split(',').filter(|p| !p.is_empty()) {
                let (id, screen) = pair.split_once(':').context("无效的 Plasma 屏幕标识")?;
                list.push(serde_json::json!({"id":id.parse::<u32>()?.to_string(),"label":format!("Display {}",screen.parse::<u32>()?+1)}));
            }
            return Ok(serde_json::to_string(&list)?);
        }
        Ok("[]".to_owned())
    }
}

pub fn apply(path: &Path, target: &WallpaperTarget) -> Result<()> {
    let image = path.canonicalize().context("壁纸文件不可用")?;
    #[cfg(windows)]
    return match target {
        WallpaperTarget::All => windows::set_wallpaper_for_all_monitors(&image),
        WallpaperTarget::Monitor(id) => windows::set_wallpaper_for_monitor(&image, id),
    };
    #[cfg(target_os = "linux")]
    return linux_apply(&image, target);
}

#[cfg(not(windows))]
fn run(command: &str, args: &[&str]) -> Result<String> {
    let result = std::process::Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("无法启动 {command}"))?;
    anyhow::ensure!(
        result.status.success(),
        "{command}: {}",
        String::from_utf8_lossy(&result.stderr).trim()
    );
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

#[cfg(target_os = "linux")]
fn plasma(script: &str) -> Result<String> {
    run(
        "gdbus",
        &[
            "call",
            "--session",
            "--dest",
            "org.kde.plasmashell",
            "--object-path",
            "/PlasmaShell",
            "--method",
            "org.kde.PlasmaShell.evaluateScript",
            script,
        ],
    )
}

#[cfg(target_os = "linux")]
fn linux_apply(image: &Path, target: &WallpaperTarget) -> Result<()> {
    let uri = url::Url::from_file_path(image).map_err(|_| anyhow::anyhow!("无效的本地壁纸路径"))?;
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if desktop.split(':').any(|part| part == "kde") {
        let id: Option<u32> = target.monitor_id().map(str::parse).transpose()?;
        let script = format!(
            r#"var id = {id}, chosen = desktopsForActivity(currentActivity()).filter(d => d.screen >= 0 && (id === null || d.id === id)); if (!chosen.length) throw Error("显示器已断开"); chosen.forEach(d => {{ d.wallpaperPlugin="org.kde.image"; d.currentConfigGroup=["Wallpaper","org.kde.image","General"]; d.writeConfig("Image",{uri}); }}); print("NSH-DONE");"#,
            id = serde_json::to_string(&id)?,
            uri = serde_json::to_string(uri.as_str())?
        );
        anyhow::ensure!(
            plasma(&script)?.contains("NSH-DONE"),
            "Plasma 未确认设置壁纸"
        );
        return Ok(());
    }
    anyhow::ensure!(target.monitor_id().is_none(), "当前桌面不支持单屏设置");
    if !desktop.split(':').any(|part| part == "gnome") {
        use ashpd::desktop::wallpaper::{SetOn, WallpaperRequest};
        use std::os::fd::AsFd;
        let file = std::fs::File::open(image)?;
        crate::runtime()?
            .block_on(async {
                WallpaperRequest::default()
                    .set_on(SetOn::Background)
                    .show_preview(true)
                    .build_file(&file.as_fd())
                    .await?
                    .response()
            })
            .context("桌面未接受设置壁纸请求")?;
        return Ok(());
    }
    for key in ["picture-uri", "picture-uri-dark"] {
        run(
            "gsettings",
            &["set", "org.gnome.desktop.background", key, uri.as_str()],
        )?;
    }
    Ok(())
}
