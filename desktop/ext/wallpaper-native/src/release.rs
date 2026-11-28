//! Select and authenticate a release from the user's selected distribution site.
use crate::region::Source;
use anyhow::{Context, Result, bail, ensure};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use reqwest::{Client, Url, redirect::Policy};
use ring::signature::{ED25519, UnparsedPublicKey};
use semver::Version;
use serde::Deserialize;
use std::time::Duration;

const LIMIT: usize = 2 * 1024 * 1024;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Attachment>,
}

#[derive(Deserialize)]
struct Attachment {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct Envelope {
    payload: String,
    signature: String,
}

#[derive(Deserialize)]
struct Manifest {
    format: u8,
    version: String,
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    os: String,
    arch: String,
    minimum_system: String,
    filename: String,
    bytes: u64,
    sha256: String,
}

fn version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}

pub(crate) fn allowed(source: Source, url: &Url) -> bool {
    let host = url.host_str().unwrap_or("");
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match source {
        Source::GitHub => {
            host == "github.com"
                || host == "api.github.com"
                || host.ends_with(".githubusercontent.com")
        }
        Source::GitCode => {
            host == "gitcode.com"
                || host.ends_with(".gitcode.com")
                || host == "gitcodeusercontent.com"
                || host.ends_with(".gitcodeusercontent.com")
        }
    }
}

async fn read(client: &Client, source: Source, path: &str) -> Result<Vec<u8>> {
    let url = Url::parse(path)?;
    ensure!(allowed(source, &url), "更新地址与已选地区不匹配");
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(body.len() + chunk.len() <= LIMIT, "发布信息过大");
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn verified(envelope: &[u8], key: &[u8], expected: &Version) -> Result<Manifest> {
    let signed: Envelope = serde_json::from_slice(envelope)?;
    let payload = STANDARD.decode(signed.payload)?;
    let signature = STANDARD.decode(signed.signature)?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&payload, &signature)
        .map_err(|_| anyhow::anyhow!("发行清单签名无效"))?;
    let manifest: Manifest = serde_json::from_slice(&payload)?;
    ensure!(
        manifest.format == 1 && Version::parse(&manifest.version)? == *expected,
        "发行清单版本不匹配"
    );
    Ok(manifest)
}

fn attachment<'a>(release: &'a Release, name: &str) -> Result<&'a str> {
    let mut matches = release.assets.iter().filter(|a| a.name == name);
    let first = matches.next().context("发行附件缺失")?;
    ensure!(matches.next().is_none(), "发行附件名称重复");
    Ok(&first.browser_download_url)
}

fn platform_package() -> Result<(&'static str, &'static str, &'static str)> {
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => ("windows", "x64", "windows-x64.exe"),
        ("linux", "x86_64") => ("linux", "x86_64", "linux-x86_64.AppImage"),
        _ => bail!("当前平台没有更新包"),
    })
}

pub async fn check(source: Source, download: bool) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(25))
        .user_agent("BingWallpaperLib/0.3")
        .redirect(Policy::custom(move |attempt| {
            if attempt.previous().len() < 8 && allowed(source, attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("更新请求重定向到了未验证的地址")
            }
        }))
        .build()?;
    let url = match source {
        Source::GitHub => {
            "https://api.github.com/repos/pandaligx/bing-wallpaper-lib/releases?per_page=50"
        }
        Source::GitCode => {
            "https://api.gitcode.com/api/v5/repos/gcw_qWIjmve7/bing-wallpaper-lib/releases?per_page=50"
        }
    };
    let releases: Vec<Release> = serde_json::from_slice(&read(&client, source, url).await?)?;
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let candidate = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| version(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, _)| v.pre.is_empty() && *v > current)
        .max_by(|a, b| a.0.cmp(&b.0));
    let Some((expected, release)) = candidate else {
        return Ok(serde_json::json!({"state":"current","provider":source.name()}).to_string());
    };
    let url = attachment(&release, "signed-release.json")?;
    let bytes = read(&client, source, url).await?;
    let key = option_env!("BING_WALLPAPER_UPDATE_PUBLIC_KEY")
        .filter(|s| !s.is_empty())
        .context("此测试构建未配置正式更新公钥")?;
    let manifest = verified(&bytes, &URL_SAFE_NO_PAD.decode(key)?, &expected)?;
    let (os, arch, suffix) = platform_package()?;
    let filename = format!("BingWallpaperLib-v{expected}-{suffix}");
    let mut candidates = manifest
        .artifacts
        .iter()
        .filter(|a| a.os == os && a.arch == arch);
    let asset = candidates.next().context("清单缺少当前平台")?;
    ensure!(
        candidates.next().is_none()
            && asset.filename == filename
            && asset.bytes > 0
            && asset.sha256.len() == 64
            && asset.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "发行附件元数据无效"
    );
    ensure!(
        allowed(source, &Url::parse(attachment(&release, &filename)?)?),
        "发行附件来自其他地区"
    );
    if download {
        use fs2::FileExt;
        let root = crate::paths::app_data_dir()?
            .join("updates")
            .join(source.name())
            .join(expected.to_string());
        std::fs::create_dir_all(&root)?;
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("update.lock"))?;
        lock.try_lock_exclusive().context("更新包正在下载")?;
        let target = root.join(&filename);
        crate::paths::write_atomic(&root.join("signed-release.json"), &bytes)?;
        let expected_bytes = asset.bytes;
        let expected_hash = asset.sha256.clone();
        let existing = target.clone();
        let digest = expected_hash.clone();
        if tokio::task::spawn_blocking(move || reuse_staged(&existing, expected_bytes, &digest))
            .await??
        {
            return Ok(
                serde_json::json!({"state":"staged","version":expected.to_string(),"path":target})
                    .to_string(),
            );
        }
        let origin = crate::transfer::probe(
            &client,
            attachment(&release, &filename)?,
            Some(expected_bytes),
        )
        .await?;
        let staging = crate::transfer::fetch(&origin, &root, "update.partial").await?;
        let path = tokio::task::spawn_blocking(move || {
            let temporary = tempfile::TempPath::try_from_path(staging)?;
            verify_file(&temporary, expected_bytes, &expected_hash)?;
            temporary.persist_noclobber(&target)?;
            Ok::<_, anyhow::Error>(target)
        })
        .await??;
        return Ok(
            serde_json::json!({"state":"staged","version":expected.to_string(),"path":path})
                .to_string(),
        );
    }
    Ok(serde_json::json!({"state":"available","version":expected.to_string(),"minimum":asset.minimum_system,"bytes":asset.bytes,"provider":source.name()}).to_string())
}

/// Copy the staged distribution artifact and authenticate the actual copy on the installation volume.
pub(crate) fn copy_verified(version_text: &str, destination: &std::path::Path) -> Result<()> {
    let expected = Version::parse(version_text)?;
    ensure!(
        expected > Version::parse(env!("CARGO_PKG_VERSION"))? && expected.pre.is_empty(),
        "无效的更新版本"
    );
    let source = crate::region::selected()?.context("请选择更新地区")?;
    let root = crate::paths::app_data_dir()?
        .join("updates")
        .join(source.name())
        .join(expected.to_string());
    let key = option_env!("BING_WALLPAPER_UPDATE_PUBLIC_KEY")
        .filter(|k| !k.is_empty())
        .context("缺少更新公钥")?;
    let envelope = std::fs::read(root.join("signed-release.json"))?;
    ensure!(envelope.len() <= LIMIT, "清单过大");
    let manifest = verified(&envelope, &URL_SAFE_NO_PAD.decode(key)?, &expected)?;
    let (os, arch, suffix) = platform_package()?;
    let name = format!("BingWallpaperLib-v{expected}-{suffix}");
    let mut rows = manifest
        .artifacts
        .iter()
        .filter(|a| a.os == os && a.arch == arch && a.filename == name);
    let asset = rows.next().context("未找到本平台附件")?;
    ensure!(rows.next().is_none() && asset.bytes > 0, "发行清单无效");
    std::fs::copy(root.join(name), destination)?;
    verify_file(destination, asset.bytes, &asset.sha256)
}

fn reuse_staged(path: &std::path::Path, length: u64, digest: &str) -> Result<bool> {
    if !path.try_exists()? {
        return Ok(false);
    }
    if matches_file(path, length, digest)? {
        return Ok(true);
    }
    // Only the owned staging copy is discarded; the installed program remains usable.
    std::fs::remove_file(path)?;
    Ok(false)
}

fn matches_file(path: &std::path::Path, length: u64, digest: &str) -> Result<bool> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)?;
    if file.metadata()?.len() != length {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    let read = std::io::copy(&mut file, &mut hash)?;
    Ok(read == length && format!("{:x}", hash.finalize()).eq_ignore_ascii_case(digest))
}

fn verify_file(path: &std::path::Path, length: u64, digest: &str) -> Result<()> {
    ensure!(
        matches_file(path, length, digest)?,
        "更新包长度或 SHA-256 不匹配"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_corrupt_or_incomplete_update_files() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("release.partial");
        std::fs::write(&path, b"original").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"original"));
        assert!(verify_file(&path, 8, &digest).is_ok());
        assert!(verify_file(&path, 9, &digest).is_err());
        std::fs::write(&path, b"modified").unwrap();
        assert!(verify_file(&path, 8, &digest).is_err());
        assert!(!reuse_staged(&path, 8, &digest).unwrap());
        assert!(!path.exists());
        std::fs::write(&path, b"original").unwrap();
        assert!(reuse_staged(&path, 8, &digest).unwrap());
        assert!(path.is_file());
    }
    use ring::{
        rand::SystemRandom,
        signature::{Ed25519KeyPair, KeyPair},
    };
    #[test]
    fn verifies_exact_manifest_bytes_and_release_tag() {
        let doc = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let payload = br#"{"format":1,"version":"1.0.0","artifacts":[]}"#;
        let sig = pair.sign(payload);
        let wrap = |bytes: &[u8]| {
            serde_json::to_vec(&serde_json::json!({"payload":STANDARD.encode(bytes),"signature":STANDARD.encode(sig.as_ref())})).unwrap()
        };
        let expected = Version::new(1, 0, 0);
        assert!(verified(&wrap(payload), pair.public_key().as_ref(), &expected).is_ok());
        assert!(verified(&wrap(b"changed"), pair.public_key().as_ref(), &expected).is_err());
        assert!(
            verified(
                &wrap(payload),
                pair.public_key().as_ref(),
                &Version::new(1, 0, 1)
            )
            .is_err()
        );
    }
    #[test]
    fn forbids_switching_providers_on_redirect() {
        let link = Url::parse("https://release-assets.githubusercontent.com/file").unwrap();
        assert!(allowed(Source::GitHub, &link));
        assert!(!allowed(Source::GitCode, &link));
        for value in [
            "https://gitcode.com.example.org/file",
            "http://gitcode.com/file",
        ] {
            assert!(!allowed(Source::GitCode, &Url::parse(value).unwrap()));
        }
    }
}
