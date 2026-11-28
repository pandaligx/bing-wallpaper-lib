//! The old program supervises a signed update until the new window opens.
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Mutex, mpsc},
    time::Duration,
};
const STAGED: &str = "BING-WALLPAPER-STAGED";
const READY: &str = "BING-WALLPAPER-WINDOW-READY";
static PIPE: Mutex<Option<ChildStdin>> = Mutex::new(None);
fn command(exe: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
fn target() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        Ok(std::env::current_exe()?)
    }
    #[cfg(target_os = "linux")]
    {
        Ok(
            PathBuf::from(std::env::var_os("APPIMAGE").context("请从 AppImage 更新")?)
                .canonicalize()?,
        )
    }

}
fn marker(child: &mut Child, expected: &str, timeout: Duration) -> Result<()> {
    let expected = expected.to_owned();
    let stdout = child.stdout.take().context("无法读取启动结果")?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if line.trim() == expected {
                let _ = tx.send(Ok(()));
                return;
            }
            if let Some(err) = line.strip_prefix("BING-WALLPAPER-ERROR ") {
                let _ = tx.send(Err(err.to_owned()));
                return;
            }
        }
    });
    rx.recv_timeout(timeout)
        .context("程序启动未确认")?
        .map_err(anyhow::Error::msg)
}
pub fn begin(version: &str) -> Result<()> {
    semver::Version::parse(version)?;
    let mut guard = PIPE.lock().map_err(|_| anyhow::anyhow!("更新正在运行"))?;
    ensure!(guard.is_none(), "更新正在运行");
    let helper = target()?;
    let mut child = command(&helper)
        .args(["--install-wallpaper-update", version])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    if let Err(error) = marker(&mut child, STAGED, Duration::from_secs(180)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let mut input = child.stdin.take().context("无法连接更新进程")?;
    input.write_all(b"1")?;
    *guard = Some(input);
    Ok(())
}
pub fn acknowledge() {
    if std::env::var_os("BING_WALLPAPER_CONFIRM_WINDOW").is_some() {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{READY} {}", env!("CARGO_PKG_VERSION"));
        let _ = out.flush();
    }
}
fn prepare(version: &str, work: &Path, current: &Path) -> Result<PathBuf> {
    let payload = work.join("download");
    crate::release::copy_verified(version, &payload)?;
    fs::set_permissions(&payload, fs::metadata(current)?.permissions())?;
    Ok(payload)
}
fn activate(
    current: &Path,
    next: &Path,
    backup: &Path,
    launch: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    ensure!(!backup.try_exists()?, "备份已存在");
    fs::rename(current, backup).context("无法备份正在运行的版本")?;
    if let Err(e) = fs::rename(next, current) {
        fs::rename(backup, current).context("无法还原旧版本")?;
        return Err(e.into());
    }
    if let Err(e) = launch(current) {
        fs::rename(current, next).context("无法移开失败的新版本")?;
        fs::rename(backup, current).context("无法还原旧版本")?;
        return Err(e);
    }
    Ok(())
}
fn run(version: &str) -> Result<()> {
    let current = target()?;
    let parent = current.parent().context("没有安装目录")?;
    let staging = tempfile::Builder::new()
        .prefix(".bing-wallpaper-upgrade-")
        .tempdir_in(parent)
        .context("安装目录不可写")?;
    let next = prepare(version, staging.path(), &current)?;
    println!("{STAGED}");
    std::io::stdout().flush()?;
    let mut input = std::io::stdin().lock();
    let mut commit = [0];
    input.read_exact(&mut commit)?;
    ensure!(commit == *b"1", "更新已取消");
    std::io::copy(&mut input, &mut std::io::sink())?;
    std::thread::sleep(Duration::from_millis(500));
    let backup = staging.path().join("previous");
    let result = activate(&current, &next, &backup, |p| {
        let mut child = command(p)
            .env("BING_WALLPAPER_CONFIRM_WINDOW", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let result = marker(
            &mut child,
            &format!("{READY} {version}"),
            Duration::from_secs(90),
        );
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    });
    if let Err(error) = result {
        if let Ok(dir) = crate::paths::config_dir() {
            let _ = crate::paths::write_atomic(
                &dir.join("last-update-error"),
                format!("{error:#}").as_bytes(),
            );
        }
        if backup.try_exists()? {
            let recovery = staging.keep();
            anyhow::bail!("{error:#} · {}", recovery.display());
        }
        if current.try_exists()? {
            command(&current).spawn()?;
        }
        return Err(error);
    }
    Ok(())
}
pub fn last_error() -> Result<String> {
    let file = crate::paths::config_dir()?.join("last-update-error");
    match fs::read_to_string(&file) {
        Ok(text) => {
            fs::remove_file(file)?;
            Ok(text)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}
pub fn supervise(version: &str) -> Result<()> {
    let result = run(version);
    if let Err(error) = &result {
        let _ = writeln!(std::io::stdout(), "BING-WALLPAPER-ERROR {error:#}");
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_launch_restores_original_and_keeps_unrelated_files() {
        let temp = tempfile::tempdir().unwrap();
        let current = temp.path().join("portable");
        let next = temp.path().join("next");
        let old = temp.path().join("backup");
        fs::write(&current, b"old").unwrap();
        fs::write(&next, b"new").unwrap();
        let other = temp.path().join("holiday.jpg");
        fs::write(&other, b"picture").unwrap();
        assert!(activate(&current, &next, &old, |_| anyhow::bail!("window failed")).is_err());
        assert_eq!(fs::read(&current).unwrap(), b"old");
        assert_eq!(fs::read(&next).unwrap(), b"new");
        activate(&current, &next, &old, |p| {
            ensure!(fs::read(p)? == b"new");
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read(&old).unwrap(), b"old");
        assert_eq!(fs::read(&other).unwrap(), b"picture");
    }
}
