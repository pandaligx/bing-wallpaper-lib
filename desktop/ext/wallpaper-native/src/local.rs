use crate::{local_thumbnails, settings::AppSettings};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::path::{Component, Path, PathBuf};

fn file(root: &Path, name: &str) -> Result<PathBuf> {
    let name_path = Path::new(name);
    let mut parts = name_path.components();
    ensure!(
        matches!(parts.next(), Some(Component::Normal(_)))
            && parts.next().is_none()
            && !name.contains(['/', '\\', ':']),
        "无效的本地图片名称"
    );
    let ext = name_path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    ensure!(
        matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp"),
        "不支持该图片格式"
    );
    let path = root.join(name_path);
    ensure!(
        std::fs::symlink_metadata(&path)?.file_type().is_file()
            && !local_thumbnails::is_downloading(&path),
        "文件不可用或仍在下载"
    );
    Ok(path)
}

pub fn files() -> Result<String> {
    let root = AppSettings::load().effective_download_dir()?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if file(&root, &name).is_ok() {
            names.push(name);
        }
    }
    names.sort_unstable_by(|a, b| b.cmp(a));
    Ok(serde_json::to_string(&names)?)
}

pub fn image(name: &str, preview: bool) -> Result<String> {
    let path = file(&AppSettings::load().effective_download_dir()?, name)?;
    let data = if preview {
        let mut reader = image::ImageReader::open(&path)?.with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_height = Some(16384);
        limits.max_image_width = Some(16384);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let img = reader.decode()?.thumbnail(1920, 1080);
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85).encode_image(&img)?;
        buf
    } else {
        let finger = local_thumbnails::fingerprint(&path)?;
        let thumb = local_thumbnails::ensure(&path, &finger)?;
        std::fs::read(thumb)?
    };
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(data)))
}

pub fn apply(name: &str) -> Result<()> {
    let settings = AppSettings::load();
    let path = file(&settings.effective_download_dir()?, name)?;
    crate::transfer::validate_image(&path)?;
    crate::platform::apply(&path, &settings.wallpaper_target)
}

pub fn delete(names: &str) -> Result<String> {
    let root = AppSettings::load().effective_download_dir()?;
    let selected: std::collections::BTreeSet<String> = serde_json::from_str(names)?;
    let mut deleted = Vec::new();
    let mut failed = Vec::new();
    for name in selected {
        let result = file(&root, &name).and_then(|path| {
            local_thumbnails::remove(&path);
            std::fs::remove_file(path).context("无法删除图片")
        });
        match result {
            Ok(_) => deleted.push(name),
            Err(e) => failed.push(serde_json::json!({"name":name,"error":e.to_string()})),
        }
    }
    Ok(serde_json::json!({"deleted":deleted,"failed":failed}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_completed_regular_pictures_in_the_selected_folder_are_eligible() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("old.jpg"), b"original").unwrap();
        std::fs::write(temp.path().join("old.jpg.aria2"), b"resume").unwrap();
        std::fs::write(temp.path().join("new.jpg"), b"image").unwrap();
        assert!(file(temp.path(), "new.jpg").is_ok());
        for name in [
            "old.jpg",
            "../new.jpg",
            "a/new.jpg",
            "file.exe",
            "new.jpg:extra.jpg",
        ] {
            assert!(file(temp.path(), name).is_err());
        }
    }
}
