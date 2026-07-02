//! 收藏壁纸持久化：以日期集合形式保存到本地 JSON。

use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::collections::HashSet;

pub fn load() -> HashSet<NaiveDate> {
    try_load().unwrap_or_default()
}

fn try_load() -> Result<HashSet<NaiveDate>> {
    let path = crate::paths::favorites_file()?;
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let data = std::fs::read(&path).context("读取收藏列表失败")?;
    let dates: Vec<NaiveDate> = serde_json::from_slice(&data).context("解析收藏列表失败")?;
    Ok(dates.into_iter().collect())
}

pub fn save(favorites: &HashSet<NaiveDate>) -> Result<()> {
    let path = crate::paths::favorites_file()?;
    let mut dates: Vec<NaiveDate> = favorites.iter().copied().collect();
    dates.sort_unstable_by(|a, b| b.cmp(a));
    let data = serde_json::to_vec_pretty(&dates).context("序列化收藏列表失败")?;
    std::fs::write(&path, data).context("写入收藏列表失败")?;
    Ok(())
}
