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

Use `nsh run --target macos` or `nsh run --target linux` on those platforms. Windows release builds use the repository's `scripts/build-nsh-windows.ps1` and embed the compiled bytecode in a portable executable. On macOS install the Intel Rust target in addition to the native Apple Silicon target and run `bash ../scripts/package-nsh-unix.sh macos`. On Linux install GTK 4.14+, WebKitGTK 6.0 and appimagetool, then run `bash ../scripts/package-nsh-unix.sh linux`. The desktop workflow builds test artifacts without publishing releases.

Windows reads the existing user archive, favourites and download settings from `%LOCALAPPDATA%/BingWallpaperLib`. The project archive provider is GitCode for mainland China and GitHub elsewhere. Images and recent records are supplied by Bing. Verify both mirrors anonymously before opening the update channel.

The migration baseline is the repository commit `e6ce419`. The root Cargo project builds the previous Windows implementation for side-by-side testing. Do not promote the prototype to the existing release channel before desktop, migration and update acceptance tests are complete.

## Data and native tasks

Changing a legacy setting preserves unknown keys and saves the original bytes once in `settings.pre-nsh.json`. The gallery reads the configured download directory. A matching intact image can be reused. A legacy `.aria2` file keeps its partial download out of the gallery. Generated thumbnails live in the application cache.

The Windows executable accepts `--daily-wallpaper` and `--scheduled-wallpaper`. The AppImage dispatches the same flags to its bundled helper. The macOS LaunchAgent runs `Contents/Helpers/bing-wallpaper-task` in the installed application. Each job takes a cross-process lock and records success after applying the image. Registration uses Windows Task Scheduler, launchd or the user's systemd instance.

## Release integrity

The client reads the selected provider's public release API. Before staging a newer package, it authenticates `signed-release.json`, checks the exact platform filename, and verifies the downloaded size and SHA-256. Windows portable replacement, macOS bundle replacement and AppImage replacement require separate upgrade and rollback acceptance.

Configure the release verification key through the build variable `BING_WALLPAPER_UPDATE_PUBLIC_KEY` (the unpadded base64url Ed25519 public key, JWK field `x`). The signer stays on the publishing machine. After uploading the same three final packages to both test releases, run the following from the repository root. The script anonymously reads every attachment from each provider before creating the identical signed manifest to upload last.

```sh
node --test scripts/sign-desktop-release.test.mjs
node scripts/sign-desktop-release.mjs target/nsh-release 0.3.0 /path/to/ed25519-private.pem
```

To reproduce the download performance comparison, build the `download_bench` example in release mode and run `node scripts/bench-download.mjs <benchmark-binary> <aria2c> [https-url]`. Omitting the URL starts the local per-connection-limited Range server. Each mode runs five times and all three modes must produce identical hashes.
