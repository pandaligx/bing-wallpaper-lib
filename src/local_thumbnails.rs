use anyhow::{bail, Context, Result};
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_VERSION: u8 = 1;
const THUMBNAIL_WIDTH: u32 = 440;
const THUMBNAIL_HEIGHT: u32 = 248;
const MAX_SOURCE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SourceFingerprint {
    source: PathBuf,
    length: u64,
    modified_nanos: Option<u128>,
}

pub fn fingerprint(source: &Path) -> Result<SourceFingerprint> {
    let metadata = std::fs::metadata(source)
        .with_context(|| format!("读取本地壁纸元数据失败: {}", source.display()))?;
    if !metadata.is_file() {
        bail!("本地壁纸不是普通文件: {}", source.display());
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());

    Ok(SourceFingerprint {
        source: source.to_path_buf(),
        length: metadata.len(),
        modified_nanos,
    })
}

fn cache_path(fingerprint: &SourceFingerprint) -> Result<PathBuf> {
    Ok(cache_path_in_dir(
        fingerprint,
        &crate::paths::downloaded_thumbnails_dir()?,
    ))
}

fn cache_path_in_dir(fingerprint: &SourceFingerprint, dir: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    CACHE_VERSION.hash(&mut hasher);
    fingerprint.hash(&mut hasher);
    dir.join(format!("{:016x}.jpg", hasher.finish()))
}

fn control_path(source: &Path) -> PathBuf {
    let mut file_name = source.file_name().map(OsString::from).unwrap_or_default();
    file_name.push(".aria2");
    source.with_file_name(file_name)
}

pub fn is_downloading(source: &Path) -> bool {
    control_path(source).exists()
}

fn cached_image_is_valid(path: &Path) -> bool {
    image::ImageReader::open(path)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.into_dimensions().ok())
        .is_some_and(|(width, height)| width > 0 && height > 0)
}

pub fn cached_for(fingerprint: &SourceFingerprint) -> Option<PathBuf> {
    let dir = crate::paths::downloaded_thumbnails_dir().ok()?;
    cached_for_in_dir(fingerprint, &dir)
}

fn cached_for_in_dir(fingerprint: &SourceFingerprint, dir: &Path) -> Option<PathBuf> {
    let path = cache_path_in_dir(fingerprint, dir);
    let exists = std::fs::metadata(&path)
        .ok()
        .is_some_and(|metadata| metadata.is_file() && metadata.len() > 0);
    if exists && cached_image_is_valid(&path) {
        Some(path)
    } else {
        if exists {
            let _ = std::fs::remove_file(&path);
        }
        None
    }
}

/// 为本地壁纸生成实际的小尺寸 JPEG 缩略图。
///
/// 调用方应在后台线程中串行调用，避免同时解码多张原图造成瞬时内存峰值。
pub fn ensure(source: &Path, expected: &SourceFingerprint) -> Result<PathBuf> {
    let dir = crate::paths::downloaded_thumbnails_dir()?;
    ensure_in_dir(source, expected, &dir)
}

fn ensure_in_dir(source: &Path, expected: &SourceFingerprint, dir: &Path) -> Result<PathBuf> {
    if is_downloading(source) {
        bail!("本地壁纸仍在下载: {}", source.display());
    }
    if fingerprint(source)? != *expected {
        bail!("本地壁纸已发生变化: {}", source.display());
    }
    if let Some(path) = cached_for_in_dir(expected, dir) {
        return Ok(path);
    }

    let target = cache_path_in_dir(expected, dir);
    let temporary = target.with_extension("tmp.jpg");
    if temporary.exists() {
        let _ = std::fs::remove_file(&temporary);
    }

    let result = (|| {
        let mut reader = image::ImageReader::open(source)
            .with_context(|| format!("打开本地壁纸失败: {}", source.display()))?
            .with_guessed_format()
            .with_context(|| format!("识别本地壁纸格式失败: {}", source.display()))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
        limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);

        let image = reader
            .decode()
            .with_context(|| format!("解码本地壁纸失败: {}", source.display()))?;
        if fingerprint(source)? != *expected || is_downloading(source) {
            bail!("生成缩略图时本地壁纸发生变化: {}", source.display());
        }

        let thumbnail = image.thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
        drop(image);
        thumbnail
            .save_with_format(&temporary, image::ImageFormat::Jpeg)
            .with_context(|| format!("保存本地壁纸缩略图失败: {}", temporary.display()))?;
        drop(thumbnail);

        if fingerprint(source)? != *expected || is_downloading(source) {
            bail!("写入缩略图时本地壁纸发生变化: {}", source.display());
        }
        if target.exists() {
            std::fs::remove_file(&target)
                .with_context(|| format!("替换本地壁纸缩略图失败: {}", target.display()))?;
        }
        std::fs::rename(&temporary, &target)
            .with_context(|| format!("写入本地壁纸缩略图失败: {}", target.display()))?;
        Ok(target.clone())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// 删除指定源文件当前版本对应的磁盘缩略图。应在删除源文件之前调用。
pub fn remove(source: &Path) {
    if let Ok(fingerprint) = fingerprint(source) {
        if let Ok(path) = cache_path(&fingerprint) {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn clear() -> Result<()> {
    let dir = crate::paths::downloaded_thumbnails_dir()?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("清空本地壁纸缩略图失败: {}", dir.display()))?;
    }
    Ok(())
}

/// 删除已不属于当前下载目录文件集合的缩略图和遗留临时文件。
pub fn prune(sources: &[PathBuf]) -> Result<()> {
    let dir = crate::paths::downloaded_thumbnails_dir()?;
    let retained: HashSet<PathBuf> = sources
        .iter()
        .filter_map(|source| fingerprint(source).ok())
        .filter_map(|fingerprint| cache_path(&fingerprint).ok())
        .collect();

    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("扫描本地壁纸缩略图失败: {}", dir.display()))?
    {
        let path = entry?.path();
        if path.is_file() && !retained.contains(&path) {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_source(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bing-wallpaper-thumbnail-{}-{suffix}.jpg",
            uuid::Uuid::new_v4()
        ))
    }

    fn test_cache_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bing-wallpaper-thumbnail-cache-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn caches_and_invalidates_thumbnail_when_source_changes() {
        let source = test_source("cache");
        let cache_dir = test_cache_dir();
        image::DynamicImage::new_rgb8(32, 18)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();
        let first_fingerprint = fingerprint(&source).unwrap();
        let first_thumbnail = ensure_in_dir(&source, &first_fingerprint, &cache_dir).unwrap();
        assert_eq!(
            cached_for_in_dir(&first_fingerprint, &cache_dir),
            Some(first_thumbnail.clone())
        );

        std::fs::OpenOptions::new()
            .append(true)
            .open(&source)
            .unwrap()
            .write_all(&[0])
            .unwrap();
        let second_fingerprint = fingerprint(&source).unwrap();
        assert_ne!(first_fingerprint, second_fingerprint);
        assert_eq!(cached_for_in_dir(&second_fingerprint, &cache_dir), None);

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn rejects_corrupt_sources_without_leaving_temporary_files() {
        let source = test_source("corrupt");
        let cache_dir = test_cache_dir();
        std::fs::write(&source, b"not an image").unwrap();
        let source_fingerprint = fingerprint(&source).unwrap();
        let target = cache_path_in_dir(&source_fingerprint, &cache_dir);
        let temporary = target.with_extension("tmp.jpg");

        assert!(ensure_in_dir(&source, &source_fingerprint, &cache_dir).is_err());
        assert!(!temporary.exists());

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn removes_corrupt_cached_thumbnail() {
        let source = test_source("invalid-cache");
        let cache_dir = test_cache_dir();
        image::DynamicImage::new_rgb8(32, 18)
            .save_with_format(&source, image::ImageFormat::Jpeg)
            .unwrap();
        let source_fingerprint = fingerprint(&source).unwrap();
        let target = cache_path_in_dir(&source_fingerprint, &cache_dir);
        std::fs::write(&target, b"invalid jpeg").unwrap();

        assert_eq!(cached_for_in_dir(&source_fingerprint, &cache_dir), None);
        assert!(!target.exists());

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_dir_all(cache_dir);
    }
}
