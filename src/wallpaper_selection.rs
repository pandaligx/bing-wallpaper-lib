use crate::model::WallpaperEntry;
use crate::settings::{AppSettings, PeriodicWallpaperSource};
use chrono::NaiveDate;
use rand::seq::SliceRandom;
use std::collections::HashSet;

#[derive(Debug)]
pub(crate) struct Selection {
    pub entry: WallpaperEntry,
    pub used_daily_latest: bool,
    pub fell_back_from_favorites: bool,
}

pub(crate) fn select_entry(
    entries: &[WallpaperEntry],
    favorites: &HashSet<NaiveDate>,
    settings: &AppSettings,
    today: NaiveDate,
) -> Option<Selection> {
    if settings.periodic_daily_first_latest && settings.last_periodic_latest_date != Some(today) {
        return entries.first().cloned().map(|entry| Selection {
            entry,
            used_daily_latest: true,
            fell_back_from_favorites: false,
        });
    }

    let mut rng = rand::thread_rng();
    match settings.periodic_wallpaper_source {
        PeriodicWallpaperSource::RandomAll => {
            entries.choose(&mut rng).cloned().map(|entry| Selection {
                entry,
                used_daily_latest: false,
                fell_back_from_favorites: false,
            })
        }
        PeriodicWallpaperSource::RandomFavorites => {
            let favorite_entries: Vec<_> = entries
                .iter()
                .filter(|entry| favorites.contains(&entry.date))
                .collect();
            favorite_entries
                .choose(&mut rng)
                .map(|entry| Selection {
                    entry: (*entry).clone(),
                    used_daily_latest: false,
                    fell_back_from_favorites: false,
                })
                .or_else(|| {
                    entries.choose(&mut rng).cloned().map(|entry| Selection {
                        entry,
                        used_daily_latest: false,
                        fell_back_from_favorites: true,
                    })
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(day: u32) -> WallpaperEntry {
        WallpaperEntry {
            date: NaiveDate::from_ymd_opt(2026, 7, day).unwrap(),
            headline: Some(format!("wallpaper-{day}")),
            title: format!("title-{day}"),
            url: format!("https://example.com/{day}.jpg"),
            copyright_link: None,
        }
    }

    #[test]
    fn first_successful_run_each_day_uses_latest_entry() {
        let entries = vec![entry(18), entry(17), entry(16)];
        let settings = AppSettings {
            periodic_daily_first_latest: true,
            last_periodic_latest_date: Some(NaiveDate::from_ymd_opt(2026, 7, 17).unwrap()),
            ..Default::default()
        };
        let selected = select_entry(
            &entries,
            &HashSet::new(),
            &settings,
            NaiveDate::from_ymd_opt(2026, 7, 18).unwrap(),
        )
        .unwrap();
        assert_eq!(selected.entry.date, entries[0].date);
        assert!(selected.used_daily_latest);
    }

    #[test]
    fn empty_favorites_fall_back_to_random_history() {
        let entries = vec![entry(18), entry(17), entry(16)];
        let settings = AppSettings {
            periodic_daily_first_latest: true,
            last_periodic_latest_date: Some(NaiveDate::from_ymd_opt(2026, 7, 18).unwrap()),
            periodic_wallpaper_source: PeriodicWallpaperSource::RandomFavorites,
            ..Default::default()
        };
        let selected = select_entry(
            &entries,
            &HashSet::new(),
            &settings,
            NaiveDate::from_ymd_opt(2026, 7, 18).unwrap(),
        )
        .unwrap();
        assert!(entries.contains(&selected.entry));
        assert!(!selected.used_daily_latest);
        assert!(selected.fell_back_from_favorites);
    }

    #[test]
    fn random_favorites_only_selects_favorited_entries() {
        let entries = vec![entry(18), entry(17), entry(16)];
        let settings = AppSettings {
            periodic_daily_first_latest: false,
            periodic_wallpaper_source: PeriodicWallpaperSource::RandomFavorites,
            ..Default::default()
        };
        let favorites = HashSet::from([entries[1].date]);
        let selected = select_entry(
            &entries,
            &favorites,
            &settings,
            NaiveDate::from_ymd_opt(2026, 7, 18).unwrap(),
        )
        .unwrap();
        assert_eq!(selected.entry.date, entries[1].date);
        assert!(!selected.fell_back_from_favorites);
    }
}
