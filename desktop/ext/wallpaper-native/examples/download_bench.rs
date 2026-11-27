//! Exercise the release engine against a specified benchmark origin in an isolated directory.
use anyhow::{Context, Result, bail, ensure};
use gosh_dl::{DownloadEngine, DownloadOptions, DownloadState, EngineConfig};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let url = args.next().context("URL required")?;
    let path = PathBuf::from(args.next().context("Output directory required")?);
    let connections: usize = args.next().context("Connection count required")?.parse()?;
    ensure!(
        (1..=16).contains(&connections),
        "Use 1 through 16 connections"
    );
    let batch = url.strip_prefix('@');
    let urls: Vec<String> = match batch {
        Some(path) => serde_json::from_slice(&std::fs::read(path)?)?,
        None => vec![url.clone()],
    };
    let engine = DownloadEngine::new(
        EngineConfig::default()
            .database_path(path.join("bench.sqlite"))
            .max_concurrent_downloads(if batch.is_some() { 4 } else { 1 }),
    )
    .await?;
    let started = Instant::now();
    let mut tasks = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        let name = if batch.is_some() {
            format!("day-{i:02}")
        } else {
            "payload".to_owned()
        };
        let id = engine
            .add_http(
                url,
                DownloadOptions::new()
                    .save_dir(&path)
                    .filename(&name)
                    .max_connections(connections),
            )
            .await?;
        tasks.push((id, name));
    }
    let mut peak = 0;
    loop {
        let mut complete = 0;
        let mut connections = 0;
        for (id, _) in &tasks {
            let status = engine.status(*id).context("Transfer disappeared")?;
            connections += status.progress.connections;
            match status.state {
                DownloadState::Completed => complete += 1,
                DownloadState::Error { message, .. } => bail!(message),
                _ => (),
            }
        }
        peak = peak.max(connections);
        if complete == tasks.len() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    println!(
        "{}",
        serde_json::json!({"elapsedMs":started.elapsed().as_millis(),"peakConnections":peak,"files":tasks.len()})
    );
    engine.shutdown().await?;
    Ok(())
}
