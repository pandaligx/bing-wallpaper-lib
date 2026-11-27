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
    let engine = DownloadEngine::new(
        EngineConfig::default()
            .database_path(path.join("bench.sqlite"))
            .max_concurrent_downloads(1),
    )
    .await?;
    let started = Instant::now();
    let id = engine
        .add_http(
            &url,
            DownloadOptions::new()
                .save_dir(&path)
                .filename("payload")
                .max_connections(connections),
        )
        .await?;
    let mut peak = 0;
    loop {
        let status = engine.status(id).context("Transfer disappeared")?;
        peak = peak.max(status.progress.connections);
        match status.state {
            DownloadState::Completed => break,
            DownloadState::Error { message, .. } => bail!(message),
            _ => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    }
    println!(
        "{}",
        serde_json::json!({"elapsedMs":started.elapsed().as_millis(),"peakConnections":peak,"bytes":std::fs::metadata(path.join("payload"))?.len()})
    );
    engine.shutdown().await?;
    Ok(())
}
