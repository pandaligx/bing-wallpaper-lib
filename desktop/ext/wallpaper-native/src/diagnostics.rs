use crate::paths;
use anyhow::Result;
use fs2::FileExt;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Once,
};

const LIMIT: u64 = 128 * 1024;
const LOG: &str = "diagnostics.log";

fn clean(text: &str) -> String {
    let mut value: String = text.chars().take(4096).collect();
    if let Some(home) = dirs::home_dir() {
        value = value.replace(home.to_string_lossy().as_ref(), "~");
    }
    value
        .split_whitespace()
        .map(|word| {
            if let Ok(url) = url::Url::parse(word.trim_matches(['(', ')', '"']))
                && let Some(host) = url.host_str()
            {
                return format!("{}://{host}", url.scheme());
            }
            if word.starts_with("ghp_") || word.starts_with("github_pat_") {
                return "[redacted]".to_owned();
            }
            word.to_owned()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn append(dir: &Path, message: &str) -> Result<()> {
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("diagnostics.lock"))?;
    lock.lock_exclusive()?;
    let current = dir.join(LOG);
    if current.metadata().is_ok_and(|m| m.len() >= LIMIT) {
        let previous = dir.join("diagnostics.previous.log");
        if previous.try_exists()? {
            fs::remove_file(&previous)?;
        }
        fs::rename(&current, previous)?;
    }
    let mut out = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(current)?;
    writeln!(
        out,
        "{} {}",
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        clean(message)
    )?;
    Ok(())
}

pub fn record(message: &str) {
    if !message.starts_with("queue:")
        && let Ok(dir) = paths::app_data_dir()
    {
        let _ = append(&dir, message);
    }
}

pub fn install_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |event| {
            record(&event.to_string());
            previous(event);
        }));
    });
}

pub fn path() -> Result<PathBuf> {
    let path = paths::app_data_dir()?.join(LOG);
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    Ok(path)
}

pub fn report() -> Result<String> {
    let lines = fs::read_to_string(path()?)?;
    let tail: Vec<_> = lines.lines().rev().take(16).collect();
    Ok(format!(
        "Bing Wallpaper Library {} · {} {}\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    ))
}

pub fn issue() -> Result<PathBuf> {
    let mut url = url::Url::parse("https://github.com/pandaligx/bing-wallpaper-lib/issues/new")?;
    let log = report()?;
    let mut tail: Vec<char> = log.chars().rev().take(900).collect();
    tail.reverse();
    let body = format!(
        "### 重现步骤 / Steps to reproduce\n\n\n### 诊断日志 / Diagnostics\n\n    {}",
        tail.into_iter().collect::<String>().replace('\n', "\n    ")
    );
    url.query_pairs_mut().append_pair("body", &body);
    Ok(PathBuf::from(url.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_rotates_and_urls_omit_credentials_and_queries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(LOG), vec![b'x'; LIMIT as usize]).unwrap();
        append(
            dir.path(),
            "GET https://user:pw@bing.com/image?token=secret\nfailed",
        )
        .unwrap();
        assert!(dir.path().join("diagnostics.previous.log").is_file());
        let line = fs::read_to_string(dir.path().join(LOG)).unwrap();
        assert!(line.contains("GET https://bing.com failed"));
        assert_eq!(line.lines().count(), 1);
    }
}
