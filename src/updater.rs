//! 检查 Gitee / GitHub Releases 上的最新版本，并支持一键下载 + 自动替换当前 exe + 重启。
//!
//! 检测逻辑：优先请求 Gitee 公开 REST API `GET /repos/{owner}/{repo}/releases/latest`，
//! 失败时回退 GitHub 公开 REST API，解析其中的 `tag_name`（形如 `v0.1.0`）与当前编译时
//! 版本号（`CARGO_PKG_VERSION`）比较。
//!
//! 实际下载 **不**在本模块中用 `http_client` 直接 GET（GitHub 的 release asset URL 会
//! 302 重定向到一个带签名参数的 `release-assets.githubusercontent.com` 地址，而 reqwest 处理
//! 这个重定向链时会经常返回 400 Bad Request），而是复用项目内置的 `aria2c.exe`，由 UI 层
//! （`ui/mod.rs::run_update_download`）通过 `Aria2Manager::add_uri_to_dir` 提交任务并轮询进度，
//! 同时推送实时下载进度条、已下/总大小、速度与剩余时间到弹窗。
//!
//! 更新逻辑：下载到本地临时目录后，写出一个小的 `.bat` 脚本负责“等待本进程退出 → 将
//! 新 exe 复制为当前目录下的新版 Release 文件名 → 重新启动 → 删除旧文件与临时文件”，以 `CREATE_NO_WINDOW` 方式启动该脚本后，调用
//! `App::quit()` 优雅退出，由脚本接管完成实际的文件替换与重启（Windows 下无法在进程运行
//! 时覆盖自身的 exe 文件，因此必须借助一个独立的辅助进程）。

use anyhow::{bail, Context, Result};
use futures::AsyncReadExt;
use http_client::{HttpClient, HttpRequestExt, Request};
use serde::{de::DeserializeOwned, Deserialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// GitHub 仓库地址，用于拼接 Releases API 与网页链接。
pub const REPO_HTML_URL: &str = "https://github.com/pandaligx/bing-wallpaper-lib";

const RELEASES_API_URL: &str =
    "https://api.github.com/repos/pandaligx/bing-wallpaper-lib/releases/latest";

const GITEE_REPO_HTML_URL: &str = "https://gitee.com/pandaligx/bing-wallpaper-lib";
const GITEE_RELEASES_API_URL: &str =
    "https://gitee.com/api/v5/repos/pandaligx/bing-wallpaper-lib/releases/latest";
const MAX_RELEASE_API_BYTES: u64 = 2 * 1024 * 1024;

/// 当前编译时的版本号（来自 `Cargo.toml` 的 `package.version`）。
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 一次"发现新版本"检测的结果。
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// 去掉了前导 `v` 的版本号，例如 `0.2.0`。
    pub version: String,
    /// Release 在网页上的地址，供"查看详情"跳转。
    pub html_url: String,
    /// 优先尝试的 `.exe` 资源地址（通常是 Gitee Release 附件；无国内附件时为 GitHub 官方地址）。
    pub download_url: String,
    /// 当优先地址失败时回退使用的备用 Release asset 地址。
    pub fallback_download_url: Option<String>,
    /// 资源文件名（用于本地临时文件命名）。
    pub asset_name: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize, Clone)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GiteeRelease {
    id: u64,
    tag_name: String,
}

#[derive(Deserialize, Clone)]
struct GiteeAsset {
    name: String,
    browser_download_url: String,
}

/// 解析形如 `"v1.2.3"` / `"1.2.3"` 的版本号为 `(major, minor, patch)`，用于比较新旧。
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn is_release_executable_name(name: &str) -> bool {
    name.starts_with("bing-wallpaper-lib-v")
        && name.ends_with("-x64.exe")
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

fn github_release_download_url(tag: &str, asset_name: &str) -> String {
    format!("{REPO_HTML_URL}/releases/download/{tag}/{asset_name}")
}

fn gitee_release_html_url(tag: &str) -> String {
    format!("{GITEE_REPO_HTML_URL}/releases/tag/{tag}")
}

fn gitee_release_by_tag_api_url(tag: &str) -> String {
    format!("https://gitee.com/api/v5/repos/pandaligx/bing-wallpaper-lib/releases/tags/{tag}")
}

fn gitee_release_assets_api_url(release_id: u64) -> String {
    format!(
        "https://gitee.com/api/v5/repos/pandaligx/bing-wallpaper-lib/releases/{release_id}/attach_files?per_page=100"
    )
}

async fn get_json<T: DeserializeOwned>(http: &Arc<dyn HttpClient>, url: &str) -> Result<T> {
    let request = Request::get(url)
        .header("Accept", "application/json")
        .follow_redirects(http_client::RedirectPolicy::FollowAll)
        .body(Default::default())
        .with_context(|| format!("构建请求失败: {url}"))?;

    let mut response = http
        .send(request)
        .await
        .with_context(|| format!("请求失败: {url}"))?;

    if !response.status().is_success() {
        bail!("接口返回错误状态码 {}: {url}", response.status());
    }

    let mut body = Vec::new();
    response
        .body_mut()
        .take(MAX_RELEASE_API_BYTES + 1)
        .read_to_end(&mut body)
        .await
        .with_context(|| format!("读取响应失败: {url}"))?;
    if body.len() as u64 > MAX_RELEASE_API_BYTES {
        bail!("接口响应超过大小限制: {url}");
    }

    serde_json::from_slice(&body).with_context(|| format!("解析响应 JSON 失败: {url}"))
}

async fn gitee_asset_for_tag(
    http: &Arc<dyn HttpClient>,
    tag: &str,
    asset_name: Option<&str>,
) -> Result<Option<GiteeAsset>> {
    let release: GiteeRelease = get_json(http, &gitee_release_by_tag_api_url(tag)).await?;
    let assets: Vec<GiteeAsset> = get_json(http, &gitee_release_assets_api_url(release.id)).await?;

    Ok(assets.into_iter().find(|asset| {
        is_release_executable_name(&asset.name) && asset_name.is_none_or(|name| asset.name == name)
    }))
}

async fn check_gitee_for_update(http: &Arc<dyn HttpClient>) -> Result<Option<ReleaseInfo>> {
    let release: GiteeRelease = get_json(http, GITEE_RELEASES_API_URL).await?;
    let (Some(latest), Some(current)) = (
        parse_version(&release.tag_name),
        parse_version(CURRENT_VERSION),
    ) else {
        return Ok(None);
    };

    if latest <= current {
        return Ok(None);
    }

    let Some(asset) = gitee_asset_for_tag(http, &release.tag_name, None).await? else {
        bail!("Gitee Release {} 未找到 exe 附件", release.tag_name);
    };

    let version = release.tag_name.trim_start_matches('v').to_string();
    let github_download_url = github_release_download_url(&release.tag_name, &asset.name);
    let fallback_download_url =
        (asset.browser_download_url != github_download_url).then_some(github_download_url);

    Ok(Some(ReleaseInfo {
        version,
        html_url: gitee_release_html_url(&release.tag_name),
        download_url: asset.browser_download_url,
        fallback_download_url,
        asset_name: asset.name,
    }))
}

async fn check_github_for_update(http: &Arc<dyn HttpClient>) -> Result<Option<ReleaseInfo>> {
    let release: GithubRelease = get_json(http, RELEASES_API_URL).await?;

    let (Some(latest), Some(current)) = (
        parse_version(&release.tag_name),
        parse_version(CURRENT_VERSION),
    ) else {
        return Ok(None);
    };

    if latest <= current {
        return Ok(None);
    }

    let Some(asset) = release
        .assets
        .iter()
        .find(|asset| is_release_executable_name(&asset.name))
        .cloned()
    else {
        bail!("GitHub Release {} 未找到 exe 附件", release.tag_name);
    };

    let version = release.tag_name.trim_start_matches('v').to_string();
    let github_download_url = asset.browser_download_url.clone();
    let gitee_download_url =
        match gitee_asset_for_tag(http, &release.tag_name, Some(&asset.name)).await {
            Ok(Some(asset)) => Some(asset.browser_download_url),
            Ok(None) => None,
            Err(err) => {
                log::warn!("Gitee Release 附件查询失败，将使用 GitHub 下载地址: {err:#}");
                None
            }
        };
    let download_url = gitee_download_url.unwrap_or_else(|| github_download_url.clone());
    let fallback_download_url =
        (download_url != github_download_url).then_some(github_download_url);

    Ok(Some(ReleaseInfo {
        version,
        html_url: release.html_url,
        download_url,
        fallback_download_url,
        asset_name: asset.name,
    }))
}

/// 检查 Gitee / GitHub 上是否已发布比当前运行版本更新的正式版本。
///
/// 返回 `Ok(Some(info))` 表示发现新版本；`Ok(None)` 表示已是最新（或无法判断）。
pub async fn check_for_update(http: Arc<dyn HttpClient>) -> Result<Option<ReleaseInfo>> {
    match check_gitee_for_update(&http).await {
        Ok(Some(info)) => Ok(Some(info)),
        Ok(None) => match check_github_for_update(&http).await {
            Ok(result) => Ok(result),
            Err(err) => {
                log::warn!("GitHub Releases 检查失败，已采用 Gitee 当前版本判断: {err:#}");
                Ok(None)
            }
        },
        Err(gitee_err) => {
            log::warn!("Gitee Releases 检查失败，将回退 GitHub: {gitee_err:#}");
            check_github_for_update(&http).await
        }
    }
}

/// 下载更新资源目录：`%LOCALAPPDATA%\BingWallpaperLib\update`。
///
/// 公开给 UI 层（`ui/mod.rs`）使用，作为 aria2 下载新版本时的 `--dir` 目标目录。
pub fn update_dir() -> Result<PathBuf> {
    let dir = crate::paths::app_data_dir()?.join("update");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 生成负责"等待旧进程退出 → 复制新版 exe → 重新启动 → 清理旧文件/临时文件"的批处理脚本内容。
fn batch_path(path: &Path) -> String {
    path.display().to_string().replace('%', "%%")
}

fn build_relaunch_script(new_exe: &Path, old_exe: &Path, final_exe: &Path) -> String {
    format!(
        "@echo off\r\n\
         chcp 65001 >nul\r\n\
         ping -n 4 127.0.0.1 >nul\r\n\
         set /a n=0\r\n\
         :retry\r\n\
         copy /Y \"{new}\" \"{final}\" >nul 2>&1\r\n\
         if not errorlevel 1 goto done\r\n\
         set /a n+=1\r\n\
         if %n% GEQ 15 goto failed\r\n\
         ping -n 2 127.0.0.1 >nul\r\n\
         goto retry\r\n\
         :done\r\n\
         start \"\" \"{final}\"\r\n\
         if /I not \"{old}\"==\"{final}\" del \"{old}\" >nul 2>&1\r\n\
         if /I not \"{new}\"==\"{final}\" del \"{new}\" >nul 2>&1\r\n\
         goto cleanup\r\n\
         :failed\r\n\
         start \"\" \"{old}\"\r\n\
         :cleanup\r\n\
         del \"%~f0\" >nul 2>&1\r\n",
        new = batch_path(new_exe),
        old = batch_path(old_exe),
        final = batch_path(final_exe),
    )
}

/// 写出并以隐藏窗口方式启动"替换 + 重启"脚本。调用方在此之后应立即调用
/// `App::quit()` 让当前进程退出，脚本会在等待期过后接管完成实际替换。
///
/// 新版本覆盖到当前 exe 的原路径，避免用户启用开机自启后，注册表仍指向已被
/// 删除的旧版本文件名。
pub fn spawn_relaunch(new_exe: &Path) -> Result<()> {
    let metadata = std::fs::metadata(new_exe).context("读取新版可执行文件信息失败")?;
    if !metadata.is_file() || metadata.len() < 1024 * 1024 {
        bail!("下载的更新文件大小异常");
    }
    let mut header = [0u8; 2];
    std::fs::File::open(new_exe)
        .context("打开新版可执行文件失败")?
        .read_exact(&mut header)
        .context("读取新版可执行文件头失败")?;
    if header != *b"MZ" {
        bail!("下载的更新文件不是有效的 Windows 可执行文件");
    }

    let old_exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    let final_exe = old_exe.clone();
    let script_path = new_exe.with_file_name("apply_update.bat");
    let script = build_relaunch_script(new_exe, &old_exe, &final_exe);
    std::fs::write(&script_path, script).context("写入更新脚本失败")?;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("cmd")
            .arg("/C")
            .arg(&script_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .context("启动更新脚本失败")?;
    }
    #[cfg(not(windows))]
    {
        let _ = &script_path;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_with_v_prefix() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("2.0.0"), Some((2, 0, 0)));
        assert_eq!(parse_version("v0.1"), Some((0, 1, 0)));
    }

    #[test]
    fn compares_versions_correctly() {
        assert!(parse_version("v0.2.0") > parse_version("v0.1.0"));
        assert!(parse_version("v0.1.1") > parse_version("v0.1.0"));
        assert!(parse_version("v1.0.0") > parse_version("v0.9.9"));
        assert!(parse_version("v0.1.0") <= parse_version("v0.1.0"));
    }

    #[test]
    fn relaunch_script_preserves_current_executable_path() {
        let new_exe = Path::new(
            r"C:\Users\me\AppData\Local\BingWallpaperLib\update\bing-wallpaper-lib-v0.2.25-x64.exe",
        );
        let old_exe = Path::new(r"C:\Users\me\Desktop\必应每日壁纸.exe");
        let script = build_relaunch_script(new_exe, old_exe, old_exe);

        assert!(script.contains(r#"copy /Y "C:\Users\me\AppData\Local\BingWallpaperLib\update\bing-wallpaper-lib-v0.2.25-x64.exe" "C:\Users\me\Desktop\必应每日壁纸.exe""#));
        assert!(script.contains(r#"start "" "C:\Users\me\Desktop\必应每日壁纸.exe""#));
        assert!(script.contains(r#"if /I not "C:\Users\me\Desktop\必应每日壁纸.exe"=="C:\Users\me\Desktop\必应每日壁纸.exe" del "C:\Users\me\Desktop\必应每日壁纸.exe""#));
    }

    #[test]
    fn accepts_only_expected_release_executable_names() {
        assert!(is_release_executable_name(
            "bing-wallpaper-lib-v0.2.30-x64.exe"
        ));
        assert!(!is_release_executable_name("checksums.exe"));
        assert!(!is_release_executable_name(
            "bing-wallpaper-lib-v0.2.30-%PATH%-x64.exe"
        ));
    }

    #[test]
    fn formats_github_release_download_url() {
        let url = github_release_download_url("v0.2.9", "bing-wallpaper-lib-v0.2.9-x64.exe");
        assert_eq!(
            url,
            "https://github.com/pandaligx/bing-wallpaper-lib/releases/download/v0.2.9/bing-wallpaper-lib-v0.2.9-x64.exe"
        );
    }
}
