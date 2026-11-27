# NSH desktop development

The desktop prototype uses Native Shell 0.7.2 (commit `0c29b5e3351128f489fe53bfb167160801707191`) with a TSX interface and an in-process Rust module. Install the matching CLI from [the official release](https://github.com/longbridge/nsh/releases/tag/v0.7.2). The source for gosh-dl 0.6.4 is locked at `9f6ff470a854be7ff49dadc609207eb53da4bdbc`. Its HTTP and storage features run on the application's Tokio runtime.

From this directory:

```sh
nsh check
nsh test
nsh run --target windows
cargo test --locked --manifest-path ext/wallpaper-native/Cargo.toml
cargo clippy --locked --manifest-path ext/wallpaper-native/Cargo.toml --all-targets -- -D warnings
```

Use `nsh run --target macos` or `nsh run --target linux` on those platforms. Windows release builds use the repository's `scripts/build-nsh-windows.ps1` and embed the compiled bytecode in a portable executable. On macOS install the Intel Rust target in addition to the native Apple Silicon target and run `nsh package --target macos --arch universal`. On Linux install GTK 4.14+, WebKitGTK 6.0 and appimagetool, then run `nsh package --target linux --format appimage`. The desktop workflow builds test artifacts without publishing releases.

Windows reads the existing user archive, favourites and download settings from `%LOCALAPPDATA%/BingWallpaperLib`. The project archive provider is GitCode for mainland China and GitHub elsewhere. Images and recent records are supplied by Bing. Verify both mirrors anonymously before opening the update channel.

The migration baseline is the repository commit `e6ce419`. The root Cargo project builds the previous Windows implementation for side-by-side testing. Do not promote the prototype to the existing release channel before desktop, migration and update acceptance tests are complete.
