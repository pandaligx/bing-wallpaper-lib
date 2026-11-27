use crate::{paths, preferences, settings::AppSettings};
use anyhow::{Context, Result, anyhow, ensure};
use fs2::FileExt;
use serde_json::Value;
use std::{fs, process::Command};

#[cfg(windows)]
#[path = "../../../../src/task_scheduler.rs"]
pub mod windows;

pub fn configure(raw: &str) -> Result<String> {
    let value: Value = serde_json::from_str(raw)?;
    let changes = value.as_object().context("无效的定时设置")?;
    let daily = changes.keys().all(|k| {
        matches!(
            k.as_str(),
            "auto_wallpaper_enabled"
                | "auto_wallpaper_source"
                | "auto_wallpaper_hour"
                | "auto_wallpaper_minute"
        )
    });
    let periodic = changes.keys().all(|k| {
        matches!(
            k.as_str(),
            "periodic_task_enabled"
                | "periodic_interval_minutes"
                | "periodic_daily_first_latest"
                | "periodic_wallpaper_source"
        )
    });
    ensure!(
        !changes.is_empty() && (daily || periodic),
        "请分别保存每日或周期设置"
    );
    let dir = paths::config_dir()?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("scheduler-config.lock"))?;
    lock.lock_exclusive()?;
    let mut value: Value = serde_json::from_str(&preferences::get()?)?;
    let previous: AppSettings = serde_json::from_value(value.clone())?;
    let object = value.as_object_mut().context("不能读取设置")?;
    for (k, v) in changes {
        object.insert(k.clone(), v.clone());
    }
    let next: AppSettings = serde_json::from_value(value)?;
    ensure!(
        next.auto_wallpaper_hour < 24
            && next.auto_wallpaper_minute < 60
            && (1..=1439).contains(&next.periodic_interval_minutes),
        "时间超出范围"
    );
    let mut changes = changes.clone();
    if !previous.periodic_task_enabled && next.periodic_task_enabled {
        changes.insert("last_periodic_latest_date".into(), Value::Null);
    }
    if (daily && !previous.auto_wallpaper_enabled && !next.auto_wallpaper_enabled)
        || (periodic && !previous.periodic_task_enabled && !next.periodic_task_enabled) {
        return preferences::edit(&dir,&changes);
    }
    let result = install(daily, &next).and_then(|_| preferences::edit(&dir, &changes));
    if let Err(error) = result {
        return match install(daily, &previous) {
            Ok(()) => Err(error),
            Err(restore) => Err(anyhow!("{error:#}；恢复原计划失败：{restore:#}")),
        };
    }
    result
}

fn run(cmd: &mut Command) -> Result<String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let result = cmd.output()?;
    ensure!(
        result.status.success(),
        "{}: {}",
        cmd.get_program().to_string_lossy(),
        String::from_utf8_lossy(&result.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

#[cfg(windows)]
fn install(daily: bool, s: &AppSettings) -> Result<()> {
    if !daily {
        if s.periodic_task_enabled {
            windows::register(s.periodic_interval_minutes)?;
            ensure!(windows::is_enabled(), "任务计划未启用");
        } else {
            windows::unregister()?;
        }
        return Ok(());
    }
    const TASK: &str = "BingWallpaperLibDailyWallpaper";
    const ENABLE: &str = r#"$ErrorActionPreference='Stop'
$action=New-ScheduledTaskAction -Execute $env:BING_TASK_PROGRAM -Argument '--daily-wallpaper'
$trigger=New-ScheduledTaskTrigger -Daily -At ([DateTime]::Today.AddHours([int]$env:BING_TASK_HOUR).AddMinutes([int]$env:BING_TASK_MINUTE))
$settings=New-ScheduledTaskSettingsSet -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -MultipleInstances IgnoreNew
$user=[System.Security.Principal.WindowsIdentity]::GetCurrent().Name
Register-ScheduledTask -TaskName $env:BING_TASK_NAME -Action $action -Trigger $trigger -Settings $settings -User $user -Force | Out-Null"#;
    const DISABLE: &str = r#"$ErrorActionPreference='Stop'
if(Get-ScheduledTask -TaskName $env:BING_TASK_NAME -ErrorAction SilentlyContinue) {Unregister-ScheduledTask -TaskName $env:BING_TASK_NAME -Confirm:$false}"#;
    let system = std::env::var_os("SystemRoot").context("找不到 Windows 系统目录")?;
    let pwsh =
        std::path::PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    run(Command::new(pwsh)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            if s.auto_wallpaper_enabled {
                ENABLE
            } else {
                DISABLE
            },
        ])
        .env("BING_TASK_PROGRAM", std::env::current_exe()?)
        .env("BING_TASK_NAME", TASK)
        .env("BING_TASK_HOUR", s.auto_wallpaper_hour.to_string())
        .env("BING_TASK_MINUTE", s.auto_wallpaper_minute.to_string()))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn install(daily: bool, s: &AppSettings) -> Result<()> {
    let (mode, enabled, argument) = if daily {
        ("daily", s.auto_wallpaper_enabled, "--daily-wallpaper")
    } else {
        ("periodic", s.periodic_task_enabled, "--scheduled-wallpaper")
    };
    let id = format!("cn.lgxng.bingwallpaperlib.{mode}");
    let uid = run(Command::new("/usr/bin/id").arg("-u"))?;
    let domain = format!("gui/{uid}");
    let label = format!("{domain}/{id}");
    let dir = dirs::home_dir()
        .context("找不到用户目录")?
        .join("Library/LaunchAgents");
    fs::create_dir_all(&dir)?;
    let plist = dir.join(format!("{id}.plist"));
    if Command::new("/bin/launchctl")
        .args(["print", &label])
        .output()?
        .status
        .success()
    {
        run(Command::new("/bin/launchctl").args(["bootout", &label]))?;
    }
    if !enabled {
        if plist.try_exists()? {
            fs::remove_file(plist)?;
        }
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let helper = exe
        .parent()
        .and_then(|p| p.parent())
        .context("无法定位 .app")?
        .join("Helpers/bing-wallpaper-task");
    ensure!(helper.is_file(), "请从已安装的 .app 配置任务");
    let helper = helper
        .to_str()
        .context("任务路径含无效字符")?
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let timing = if daily {
        format!(
            "<key>StartCalendarInterval</key><dict><key>Hour</key><integer>{}</integer><key>Minute</key><integer>{}</integer></dict>",
            s.auto_wallpaper_hour, s.auto_wallpaper_minute
        )
    } else {
        format!(
            "<key>StartInterval</key><integer>{}</integer><key>RunAtLoad</key><true/>",
            u32::from(s.periodic_interval_minutes) * 60
        )
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>Label</key><string>{id}</string><key>ProgramArguments</key><array><string>{helper}</string><string>{argument}</string></array>{timing}</dict></plist>"#
    );
    paths::write_atomic(&plist, xml.as_bytes())?;
    run(Command::new("/bin/launchctl")
        .args(["bootstrap", &domain])
        .arg(plist))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn install(daily: bool, s: &AppSettings) -> Result<()> {
    let (mode, enabled, argument) = if daily {
        ("daily", s.auto_wallpaper_enabled, "--daily-wallpaper")
    } else {
        ("periodic", s.periodic_task_enabled, "--scheduled-wallpaper")
    };
    let dir = dirs::config_dir()
        .context("找不到 XDG 配置目录")?
        .join("systemd/user");
    fs::create_dir_all(&dir)?;
    let unit = format!("bing-wallpaper-{mode}");
    let timer = format!("{unit}.timer");
    let service = format!("{unit}.service");
    if !enabled {
        if dir.join(&timer).try_exists()? {
            run(Command::new("systemctl").args(["--user", "disable", "--now", &timer]))?;
            fs::remove_file(dir.join(&timer))?;
        }
        if dir.join(&service).try_exists()? {
            fs::remove_file(dir.join(service))?;
        }
        run(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
        return Ok(());
    }
    let exe =
        std::path::PathBuf::from(std::env::var_os("APPIMAGE").context("请从 AppImage 配置任务")?)
            .canonicalize()?;
    let exe = exe.to_str().context("任务路径含无效字符")?;
    ensure!(!exe.contains(['\n', '\r']), "任务路径含换行符");
    let program = exe
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
        .replace('$', "$$");
    let service_text = format!(
        "[Unit]\nDescription=Bing wallpaper {mode}\nAfter=graphical-session.target\n[Service]\nType=oneshot\nExecStart=\"{program}\" {argument}\n"
    );
    let timing = if daily {
        format!(
            "OnCalendar=*-*-* {:02}:{:02}:00\nPersistent=true",
            s.auto_wallpaper_hour, s.auto_wallpaper_minute
        )
    } else {
        format!(
            "OnStartupSec=10s\nOnUnitActiveSec={}min",
            s.periodic_interval_minutes
        )
    };
    let timer_text = format!(
        "[Unit]\nDescription=Bing wallpaper {mode} timer\n[Timer]\n{timing}\nAccuracySec=1s\n[Install]\nWantedBy=timers.target\n"
    );
    paths::write_atomic(&dir.join(service), service_text.as_bytes())?;
    paths::write_atomic(&dir.join(&timer), timer_text.as_bytes())?;
    run(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    run(Command::new("systemctl").args(["--user", "enable", &timer]))?;
    run(Command::new("systemctl").args(["--user", "restart", &timer]))?;
    Ok(())
}
