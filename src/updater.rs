//! 检查 GitHub Releases 上的最新版本，并支持一键下载 + 自动替换当前 exe + 重启。
//!
//! 检测逻辑：请求 GitHub 公开 REST API `GET /repos/{owner}/{repo}/releases/latest`，
//! 解析其中的 `tag_name`（形如 `v0.1.0`）与当前编译时版本号（`CARGO_PKG_VERSION`）比较。
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
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// GitHub 仓库地址，用于拼接 Releases API 与网页链接。
pub const REPO_HTML_URL: &str = "https://github.com/pandaligx/bing-wallpaper-lib";

const RELEASES_API_URL: &str =
    "https://api.github.com/repos/pandaligx/bing-wallpaper-lib/releases/latest";

/// 可选的更新包镜像直链模板。
///
/// 留空时仍使用 GitHub Release asset 地址下载。若需要避开 GitHub asset 下载不稳定的问题，
/// 可以在构建时设置环境变量 `BING_WALLPAPER_UPDATE_MIRROR_URL`，或把下面的常量改成你的
/// 永久直链模板。支持以下占位符：
///
/// - `{version}`：不带 `v` 的版本号，例如 `0.2.9`
/// - `{tag}`：带 `v` 的版本号，例如 `v0.2.9`
/// - `{asset}`：GitHub Release 里的 exe 文件名，例如 `bing-wallpaper-lib-v0.2.9-x64.exe`
///
/// 示例：`https://www.lgxng.cn/1814328088/g/new/soft/{asset}`
///
/// 如果你的网盘/对象存储使用固定永久链接（每次覆盖同一个文件），也可以直接填完整 URL，
/// 不使用任何占位符。
const UPDATE_MIRROR_URL_TEMPLATE: &str = "https://www.lgxng.cn/1814328088/g/new/soft/{asset}";

/// 当前编译时的版本号（来自 `Cargo.toml` 的 `package.version`）。
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 一次"发现新版本"检测的结果。
#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    /// 去掉了前导 `v` 的版本号，例如 `0.2.0`。
    pub version: String,
    /// Release 在网页上的地址，供"查看详情"跳转。
    pub html_url: String,
    /// 优先尝试的 `.exe` 资源地址（通常是镜像直链；无镜像时为 GitHub 官方地址）。
    pub download_url: String,
    /// 当优先地址失败时回退使用的 GitHub 官方 Release asset 地址。
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

/// 解析形如 `"v1.2.3"` / `"1.2.3"` 的版本号为 `(major, minor, patch)`，用于比较新旧。
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn mirror_template() -> Option<&'static str> {
    option_env!("BING_WALLPAPER_UPDATE_MIRROR_URL")
        .map(str::trim)
        .filter(|template| !template.is_empty())
        .or_else(|| {
            let template = UPDATE_MIRROR_URL_TEMPLATE.trim();
            (!template.is_empty()).then_some(template)
        })
}

fn format_mirror_url(template: &str, version: &str, asset_name: &str) -> String {
    template
        .replace("{version}", version)
        .replace("{tag}", &format!("v{version}"))
        .replace("{asset}", asset_name)
}

fn mirror_download_url(version: &str, asset_name: &str) -> Option<String> {
    mirror_template().map(|template| format_mirror_url(template, version, asset_name))
}

/// 检查 GitHub 上是否已发布比当前运行版本更新的正式版本。
///
/// 返回 `Ok(Some(info))` 表示发现新版本；`Ok(None)` 表示已是最新（或无法判断）。
pub async fn check_for_update(http: Arc<dyn HttpClient>) -> Result<Option<ReleaseInfo>> {
    let request = Request::get(RELEASES_API_URL)
        .header("Accept", "application/vnd.github+json")
        .follow_redirects(http_client::RedirectPolicy::FollowAll)
        .body(Default::default())
        .context("构建更新检查请求失败")?;

    let mut response = http
        .send(request)
        .await
        .context("请求 GitHub Releases 接口失败")?;

    if !response.status().is_success() {
        bail!("GitHub Releases 接口返回错误状态码: {}", response.status());
    }

    let mut body = Vec::new();
    response
        .body_mut()
        .read_to_end(&mut body)
        .await
        .context("读取更新检查响应失败")?;

    let release: GithubRelease =
        serde_json::from_slice(&body).context("解析 GitHub Releases 响应 JSON 失败")?;

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
        .find(|a| a.name.ends_with(".exe"))
        .or_else(|| release.assets.first())
        .cloned()
    else {
        return Ok(None);
    };

    let version = release.tag_name.trim_start_matches('v').to_string();
    let github_download_url = asset.browser_download_url.clone();
    let download_url =
        mirror_download_url(&version, &asset.name).unwrap_or_else(|| github_download_url.clone());
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

/// 下载更新资源目录：`%LOCALAPPDATA%\BingWallpaperLib\update`。
///
/// 公开给 UI 层（`ui/mod.rs`）使用，作为 aria2 下载新版本时的 `--dir` 目标目录。
pub fn update_dir() -> Result<PathBuf> {
    let dir = crate::paths::app_data_dir()?.join("update");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 生成负责"等待旧进程退出 → 复制新版 exe → 重新启动 → 清理旧文件/临时文件"的批处理脚本内容。
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
        new = new_exe.display(),
        old = old_exe.display(),
        final = final_exe.display(),
    )
}

/// 写出并以隐藏窗口方式启动"替换 + 重启"脚本。调用方在此之后应立即调用
/// `App::quit()` 让当前进程退出，脚本会在等待期过后接管完成实际替换。
///
/// 新版本会使用 Release asset 的文件名复制到当前 exe 所在目录；这样即使用户从
/// `bing-wallpaper-lib-v0.2.20-x64.exe` 或自定义的 `必应每日壁纸.exe` 启动，更新后
/// 也会变成类似 `bing-wallpaper-lib-v0.2.25-x64.exe` 的新版文件名。
pub fn spawn_relaunch(new_exe: &Path) -> Result<()> {
    let old_exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    let asset_name = new_exe.file_name().context("获取新版可执行文件名失败")?;
    let final_exe = old_exe.with_file_name(asset_name);
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
    fn relaunch_script_uses_new_release_file_name() {
        let new_exe = Path::new(
            r"C:\Users\me\AppData\Local\BingWallpaperLib\update\bing-wallpaper-lib-v0.2.25-x64.exe",
        );
        let old_exe = Path::new(r"C:\Users\me\Desktop\必应每日壁纸.exe");
        let final_exe = Path::new(r"C:\Users\me\Desktop\bing-wallpaper-lib-v0.2.25-x64.exe");
        let script = build_relaunch_script(new_exe, old_exe, final_exe);

        assert!(script.contains(r#"copy /Y "C:\Users\me\AppData\Local\BingWallpaperLib\update\bing-wallpaper-lib-v0.2.25-x64.exe" "C:\Users\me\Desktop\bing-wallpaper-lib-v0.2.25-x64.exe""#));
        assert!(
            script.contains(r#"start "" "C:\Users\me\Desktop\bing-wallpaper-lib-v0.2.25-x64.exe""#)
        );
        assert!(script.contains(r#"del "C:\Users\me\Desktop\必应每日壁纸.exe""#));
    }

    #[test]
    fn formats_mirror_download_url_template() {
        let url = format_mirror_url(
            "https://download.example.com/{tag}/{asset}?version={version}",
            "0.2.9",
            "bing-wallpaper-lib-v0.2.9-x64.exe",
        );
        assert_eq!(
            url,
            "https://download.example.com/v0.2.9/bing-wallpaper-lib-v0.2.9-x64.exe?version=0.2.9"
        );
    }
}
