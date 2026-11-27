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

The client reads the selected provider's public release API. Before staging a newer package, it authenticates `signed-release.json`, checks the platform filename, and verifies the downloaded size and SHA-256. On installation a supervisor from the running version verifies a second copy on the installation volume. It keeps the previous executable or bundle until the new process opens its offline library. A startup failure restores the old installation. macOS also checks the bundle signature and Gatekeeper assessment. Real distribution-channel upgrade and rollback results belong in the validation record.

Configure the release verification key through the build variable `BING_WALLPAPER_UPDATE_PUBLIC_KEY` (the unpadded base64url Ed25519 public key, JWK field `x`). The signer stays on the publishing machine. After uploading the same three final packages to both test releases, run the following from the repository root. The script anonymously reads every attachment from each provider before creating the identical signed manifest to upload last.

```sh
node --test scripts/sign-desktop-release.test.mjs
node scripts/sign-desktop-release.mjs target/nsh-release 0.3.0 /path/to/ed25519-private.pem
```

To reproduce the download performance comparison, build the `download_bench` example in release mode and run `node scripts/bench-download.mjs <benchmark-binary> <aria2c> [https-url]`. Omitting the URL starts the local per-connection-limited Range server. Each mode runs five times and all three modes must produce identical hashes.

On Windows, `pwsh ./scripts/bench-month.ps1 -Engine <benchmark-binary> -Aria <aria2c> -Month 2026-02` measures a full month, including peak working set and each file's digest. The migration test runs with copies of settings, favorites, unfinished transfers and images under a temporary profile. From the repository root, run `node scripts/test-nsh-migration.mjs` with NSH on PATH or its absolute path in `NSH_BIN`.

`pwsh ./scripts/test-windows-task-registration.ps1` registers two uniquely named Task Scheduler entries, checks their triggers, moves the test executable, runs both actions and removes the entries. `node scripts/rehearse-windows-update.mjs` builds private Windows test versions for a signed upgrade and rollback. Use the resulting executable with `scripts/test-windows-resident.ps1 -RehearsalExe <path>` to exercise background launch, close to tray and single-instance activation. The AppImage upgrade rehearsal runs in a disposable XDG desktop session in CI.

For a Developer ID build, set `MACOS_DEVELOPER_ID` to the signing identity and `MACOS_NOTARY_PROFILE` to the local notarytool keychain profile. The packaging script signs the app and helper, notarizes and staples the app, then signs, notarizes and staples the DMG.

For CI distribution builds, dispatch **NSH native desktop acceptance** on the matching version tag with **distribution** enabled. Configure the repository variable `BING_WALLPAPER_UPDATE_PUBLIC_KEY` for all three platforms. In the protected `desktop-release` environment provide `MACOS_CERTIFICATE_BASE64` (Developer ID Application PKCS#12), `MACOS_CERTIFICATE_PASSWORD` and `MACOS_NOTARY_KEY` (App Store Connect private key) as secrets. Set the environment variables `MACOS_DEVELOPER_ID`, `MACOS_NOTARY_KEY_ID` and `MACOS_NOTARY_ISSUER`. The publishing workflow takes the resulting acceptance run ID and redistributes those tested artifacts.

For existing public release tags on both sites, `node scripts/publish-desktop-assets.mjs ARTIFACT_DIR VERSION PRIVATE_KEY_FILE` uploads the same three files, verifies anonymous downloads, then publishes the signed manifest last. It reads `GH_TOKEN` and `GITCODE_TOKEN` on the publishing machine. Re-running it compares existing artifacts before signing. Keep private keys and publishing tokens outside the project.
