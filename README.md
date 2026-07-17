# Bing Daily Wallpaper Library

**English** | [简体中文](README.zh-CN.md)

<p align="center">
  <img src="docs/screenshot-home.png" alt="Bing Daily Wallpaper Library home screen" width="820" />
</p>

<p align="center">
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="release" src="https://img.shields.io/github/v/release/pandaligx/bing-wallpaper-lib"></a>
  <a href="https://github.com/pandaligx/bing-wallpaper-lib/releases/latest"><img alt="downloads" src="https://img.shields.io/github/downloads/pandaligx/bing-wallpaper-lib/total"></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-Windows-0078D6?logo=windows&logoColor=white">
  <img alt="language" src="https://img.shields.io/badge/language-Rust-DEA584?logo=rust&logoColor=white">
  <img alt="license" src="https://img.shields.io/badge/license-GPL--3.0-blue">
</p>

A native Windows application built with Rust, GPUI, and gpui-component. It combines the complete historical archive from [zxyongyo/bing-daily-wallpaper](https://github.com/zxyongyo/bing-daily-wallpaper) with recent data from Bing, then lets you browse, favorite, download, and set daily wallpapers.

## Features

- Complete Bing wallpaper history grouped by year and month, with a bundled offline snapshot and verified corrections for missing or damaged archive records.
- Multilingual interface with system-language detection plus English, Simplified Chinese, Japanese, Korean, Russian, and French.
- Global download resolution: original UHD, 4K, 2K, or 1K.
- Virtualized home grid, large-image preview, favorites, and a downloaded-wallpaper gallery.
- Fast multi-connection downloads through the bundled aria2 JSON-RPC engine.
- Batch downloads for all history, the selected month, favorites, or a date range.
- One-click wallpaper setting with all-monitor and individual-monitor targets.
- Scheduled daily wallpaper changes, startup launch, background mode, and a system tray menu.
- Light, dark, or system theme and automatic update checks through Gitee and GitHub Releases.
- A single statically linked executable with no separate Visual C++ runtime or aria2 installation required.

Use the flag button next to the Settings button in the lower-left sidebar to change the interface language. The selection is saved and applied immediately.

## Download

Users in mainland China can download the latest `bing-wallpaper-lib-vX.Y.Z-x64.exe` from [Gitee Releases](https://gitee.com/pandaligx/bing-wallpaper-lib/releases). The same executable is also available from [GitHub Releases](https://github.com/pandaligx/bing-wallpaper-lib/releases/latest).

Run the downloaded executable directly. The application requests administrator permission at startup and does not require an installer.

## Build from source

Requirements:

- Stable Rust with the `x86_64-pc-windows-msvc` target.
- Visual Studio Build Tools with Desktop development with C++ and a Windows 10/11 SDK.

```powershell
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The release executable is written to `target/x86_64-pc-windows-msvc/release/bing-wallpaper-lib.exe`.

## Wallpaper data

Historical metadata comes from the upstream `map.json` file and is synchronized every four hours into [`assets/data/zxyongyo-bing-wallpaper.json`](assets/data/zxyongyo-bing-wallpaper.json). Runtime loading prefers the Gitee mirror and falls back to jsDelivr and GitHub. Recent entries are strengthened with Bing's `HPImageArchive.aspx` API.

Verified archive corrections are kept separately in [`assets/data/zxyongyo-bing-wallpaper-corrections.json`](assets/data/zxyongyo-bing-wallpaper-corrections.json) so later upstream synchronization cannot reintroduce missing or malformed records. This includes the repaired January 26, 2024 Hawk Owl wallpaper.

Wallpaper copyrights belong to Bing, the photographers, and their respective copyright holders.

## License

The application is released under the repository's [GPL-3.0 license](LICENSE). The bundled, unmodified aria2 executable is distributed under [GPL-2.0](https://github.com/aria2/aria2/blob/master/COPYING) and is controlled only through its public JSON-RPC interface.

© 2023-2026 小南瓜
