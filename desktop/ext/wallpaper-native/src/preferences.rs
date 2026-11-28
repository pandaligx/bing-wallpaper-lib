use crate::{paths, settings::AppSettings};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

fn read(dir: &Path) -> Result<(Option<Vec<u8>>, Value)> {
    match fs::read(dir.join("settings.json")) {
        Ok(bytes) => {
            let value: Value = serde_json::from_slice(&bytes).context("现有配置无法解析")?;
            ensure!(value.is_object(), "现有配置格式错误");
            Ok((Some(bytes), value))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((None, json!({}))),
        Err(e) => Err(e.into()),
    }
}

pub fn get() -> Result<String> {
    let (_, value) = read(&paths::config_dir()?)?;
    let settings: AppSettings = serde_json::from_value(value)?;
    Ok(serde_json::to_string(&settings)?)
}

pub fn update(patch: &str) -> Result<String> {
    let value: Value = serde_json::from_str(patch)?;
    let changes = value.as_object().context("无效的设置")?;
    for key in changes.keys() {
        ensure!(
            matches!(
                key.as_str(),
                "download_resolution"
                    | "wallpaper_target"
                    | "download_dir"
                    | "theme_preference"
                    | "smooth_wheel"
                    | "startup_enabled"
                    | "background_resident_enabled"
                    | "auto_wallpaper_exit_after_done"
                    | "language"
            ),
            "不支持修改此设置"
        );
    }
    edit(&paths::config_dir()?, changes)
}

pub(crate) fn edit(dir: &Path, changes: &serde_json::Map<String, Value>) -> Result<String> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("settings.lock"))?;
    lock.lock_exclusive()?;
    let (original, mut value) = read(dir)?;
    let object = value.as_object_mut().context("现有配置格式错误")?;
    for (key, v) in changes {
        object.insert(key.clone(), v.clone());
    }
    let settings: AppSettings = serde_json::from_value(value.clone())?;
    if changes.contains_key("download_dir")
        && let Some(path) = &settings.download_dir
    {
        ensure!(path.is_absolute(), "请选择绝对路径");
        fs::create_dir_all(path)?;
        tempfile::NamedTempFile::new_in(path).context("下载目录无法写入")?;
    }
    if let Some(bytes) = original {
        let backup = dir.join("settings.pre-nsh.json");
        if !backup.try_exists()? {
            let mut temp = tempfile::NamedTempFile::new_in(dir)?;
            temp.write_all(&bytes)?;
            temp.as_file().sync_all()?;
            temp.persist_noclobber(backup)?;
        }
    }
    paths::write_atomic(
        &dir.join("settings.json"),
        &serde_json::to_vec_pretty(&value)?,
    )?;
    Ok(serde_json::to_string(&settings)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_is_byte_identical_and_repeat_edits_preserve_unknown_fields() {
        let temp = tempfile::tempdir().unwrap();
        let original = br#"{ "startup_enabled": true, "future_setting": 9 }"#;
        fs::write(temp.path().join("settings.json"), original).unwrap();
        for resolution in ["two_k", "one_k"] {
            let patch = json!({"download_resolution":resolution});
            edit(temp.path(), patch.as_object().unwrap()).unwrap();
        }
        let (_, value) = read(temp.path()).unwrap();
        assert_eq!(value["future_setting"], 9);
        assert_eq!(value["startup_enabled"], true);
        assert_eq!(value["download_resolution"], "one_k");
        assert_eq!(
            fs::read(temp.path().join("settings.pre-nsh.json")).unwrap(),
            original
        );
    }
}
