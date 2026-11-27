use crate::{
    archive, catalog, favorites, paths, platform, preferences, region,
    settings::{AppSettings, AutoWallpaperSource, PeriodicWallpaperSource},
    transfer, wallpaper_selection,
};
use anyhow::{Context, Result, ensure};
use chrono::{Local, Timelike};
use fs2::FileExt;
use serde_json::json;
use std::{fs, time::Duration};

pub(crate) fn task_lock(root: &std::path::Path) -> Result<fs::File> {
    Ok(fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("automatic-wallpaper.lock"))?)
}

fn periodic_due(now: i64, last: i64, interval_minutes: u16) -> bool {
    // Calendar events can arrive either side of the previous event's second within its minute.
    let minutes = now.div_euclid(60) - last.div_euclid(60);
    minutes < 0 || minutes >= i64::from(interval_minutes)
}

pub fn run(argument: &str) -> Result<()> {
    let daily = match argument {
        "--daily-wallpaper" | "--change-now" => true,
        "--scheduled-wallpaper" => false,
        _ => anyhow::bail!("未知的定时任务参数"),
    };
    let root = paths::config_dir()?;
    let lock = task_lock(&root)?;
    match lock.try_lock_exclusive() {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    let now = Local::now();
    let today = now.date_naive();
    let settings: AppSettings = serde_json::from_str(&preferences::get()?)?;
    let manual = argument == "--change-now";
    if daily && !manual {
        if !settings.auto_wallpaper_enabled || settings.last_auto_wallpaper_date == Some(today) {
            return Ok(());
        }
        if (now.hour(), now.minute())
            < (
                u32::from(settings.auto_wallpaper_hour),
                u32::from(settings.auto_wallpaper_minute),
            )
        {
            return Ok(());
        }
    } else if !daily {
        if !settings.periodic_task_enabled {
            return Ok(());
        }
        ensure!(
            (1..=1439).contains(&settings.periodic_interval_minutes),
            "无效的周期"
        );
        let last = root.join("periodic-run-time");
        if let Ok(timestamp) = fs::read_to_string(last)
            && let Ok(epoch) = timestamp.parse::<i64>()
            && !periodic_due(now.timestamp(), epoch, settings.periodic_interval_minutes)
        {
            return Ok(());
        }
    }

    let runtime = crate::runtime()?;
    if let Some(source) = region::selected()? {
        let _ = runtime.block_on(catalog::refresh(source));
    }
    let entries = archive::local_entries();
    let saved = favorites()?.into_iter().collect();
    let mut selection_settings = settings.clone();
    if daily {
        selection_settings.periodic_daily_first_latest =
            settings.auto_wallpaper_source == AutoWallpaperSource::Latest;
        selection_settings.last_periodic_latest_date = None;
        selection_settings.periodic_wallpaper_source = match settings.auto_wallpaper_source {
            AutoWallpaperSource::RandomFavorites => PeriodicWallpaperSource::RandomFavorites,
            _ => PeriodicWallpaperSource::RandomAll,
        };
    }
    let choice = wallpaper_selection::select_entry(&entries, &saved, &selection_settings, today)
        .context("没有可用的壁纸")?;
    if choice.fell_back_from_favorites {
        log::info!("收藏为空，已使用历史壁纸");
    }
    let target = settings.wallpaper_target.clone();
    let path = runtime.block_on(async {
        tokio::time::timeout(
            Duration::from_secs(600),
            transfer::download(choice.entry, settings),
        )
        .await?
    })?;
    platform::apply(&path, &target)?;
    if manual {
        return Ok(());
    }
    let change = if daily {
        json!({"last_auto_wallpaper_date":today})
    } else if choice.used_daily_latest {
        json!({"last_periodic_latest_date":today})
    } else {
        json!({})
    };
    if let Some(patch) = change.as_object().filter(|v| !v.is_empty()) {
        preferences::edit(&root, patch)?;
    }
    if !daily {
        paths::write_atomic(
            &root.join("periodic-run-time"),
            now.timestamp().to_string().as_bytes(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::periodic_due;

    #[test]
    fn periodic_wall_clock_includes_suspended_time_and_coalesces_missed_intervals() {
        let start = 6003;
        assert!(!periodic_due(6059, start, 1));
        assert!(periodic_due(6060, start, 1));
        assert!(!periodic_due(6060, start, 1439));
        let resumed = start + 3 * 1439 * 60;
        assert!(periodic_due(resumed, start, 1439));
        assert!(!periodic_due(resumed + 60, resumed, 1439));
        assert!(periodic_due(start - 3600, start, 60));
    }
}
