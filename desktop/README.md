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

## 当前验收和交付顺序

| 顺序 | 验收内容 | 交付条件 |
| --- | --- | --- |
| 1 | 固定 NSH CLI、运行时和项目内两份技能。用 Windows 单 exe、通用 Mac 应用与 AppImage 走通离线首页 → 1080p 预览 → Rust 下载 → 设置桌面 | 对照 0.2.37 的归档、纠错、收藏、分辨率与定时规则。用旧用户目录副本重复迁移，保留原图并排除 aria2 半成品 |
| 2 | 用官方 Gallery、Desktop、Tasks 的导航与工具栏模式复核五个页面 | 12 号正文、图标提示、键盘焦点、删除数量、失败重试、六种语言和窄屏 |
| 3 | Windows 以同一显示器和归档分别测普通滚轮与精细滚轮 | 记录显示器 Hz、NSH 帧数和画廊像素变化帧数，保存前后记录。另验 1809、混合 DPI 双屏和实际睡眠恢复 |
| 4 | 比较单连接、进程内下载和旧 aria2；覆盖忽略 Range、断线续传、磁盘写入失败和文件校验 | 每组跑五轮取中位数并验哈希。受控的大文件要求达到旧引擎吞吐的 90%；真实 Bing 整月样本单列，复查已记录的约 53% 差距 |
| 5 | 重新打包当前界面并在两个 Mac 架构、GNOME 和 KDE 运行 | 检查同一 DMG、AppImage 的首页/预览/设置、菜单、托盘、定时与真实桌面；按平台存明暗截图 |
| 6 | 交付本地未签名 Windows 文件给维护者，保留签名前副本 | 维护者签名后核验 Authenticode；macOS 校验 Developer ID、公证和装入「应用程序」后的升级 |
| 7 | 在本地用临时密钥演练失败回滚。维护者明确批准公开发布后，使用最终签名文件计算长度和 SHA-256 | 两站上传同一批文件，匿名回读并验证，再发布更新清单。分别验证大陆只访问 GitCode、海外只访问 GitHub 的发行链路 |

仓库手动发布工作流接受维护者提供的已签 Windows 附件 SHA-256，使用 Windows 验证 Authenticode 后才继续。签名之前的测试摘要仅用于本地比对。普通构建与界面验收使用本地输出目录或 CI 构建产物。

在仓库根目录执行以下命令可构建独立的本地 Windows 测试版，并测量正常启动后的画廊滚动。屏幕采样需要可见的原生窗口。指定 `-InputMode precision` 可改测细粒度滚动输入。

```powershell
./scripts/build-nsh-windows.ps1 -OutputDirectory target/nsh-local/windows
./scripts/measure-windows-scroll.ps1 -Exe target/nsh-local/windows/BingWallpaperLib-v0.3.0-windows-x64.exe -OutputDir target/nsh-validation/scroll -NoFrameMonitor
```

## Release integrity

The client reads the selected provider's public release API. Before staging a newer package, it authenticates `signed-release.json`, checks the platform filename, and verifies the downloaded size and SHA-256. On installation a supervisor from the running version verifies a second copy on the installation volume. It keeps the previous executable or bundle until the new process opens its offline library. A startup failure restores the old installation. macOS also checks the bundle signature and Gatekeeper assessment. Real distribution-channel upgrade and rollback results belong in the validation record.

Configure the release verification key through the build variable `BING_WALLPAPER_UPDATE_PUBLIC_KEY` (the unpadded base64url Ed25519 public key, JWK field `x`). The signer stays on the publishing machine. After uploading the same three final packages to both test releases, run the following from the repository root. The script anonymously reads every attachment from each provider before creating the identical signed manifest to upload last.

```sh
node --test scripts/sign-desktop-release.test.mjs
node scripts/sign-desktop-release.mjs target/nsh-release 0.3.0 /path/to/test-ed25519-private.pem v0.3.0-rc.1
```

For a production release, omit the final tag argument and use the matching production key. The verifier requires the exact public release channel on both sites and rejects redirects outside the selected provider.

After publishing the test envelope, run `node scripts/test-public-update.mjs v0.3.0-rc.1 /path/to/test-public-key.txt`. This creates a private predecessor build that selects only that prerelease, uses the native updater to download from each chosen site, and authenticates the staged file again on the installation volume.

To reproduce the download performance comparison, build the `download_bench` example in release mode and run `node scripts/bench-download.mjs <benchmark-binary> <aria2c> [https-url]`. Omitting the URL starts the local per-connection-limited Range server. Each mode runs five times and all three modes must produce identical hashes.

On Windows, `pwsh ./scripts/bench-month.ps1 -Engine <benchmark-binary> -Aria <aria2c> -Month 2026-02` measures a full month, including peak working set and each file's digest. The migration test runs with copies of settings, favorites, unfinished transfers and images under a temporary profile. From the repository root, run `node scripts/test-nsh-migration.mjs` with NSH on PATH or its absolute path in `NSH_BIN`.

`pwsh ./scripts/test-windows-task-registration.ps1` registers two uniquely named Task Scheduler entries, checks their triggers, moves the test executable, runs both actions and removes the entries. `node scripts/rehearse-windows-update.mjs` builds private Windows test versions for a signed upgrade and rollback. Use the resulting executable with `scripts/test-windows-resident.ps1 -RehearsalExe <path>` to exercise background launch, close to tray and single-instance activation. The AppImage upgrade rehearsal runs in a disposable XDG desktop session in CI.

For a Developer ID build, set `MACOS_DEVELOPER_ID` to the signing identity and `MACOS_NOTARY_PROFILE` to the local notarytool keychain profile. The packaging script signs the app and helper, notarizes and staples the app, then signs, notarizes and staples the DMG.

Run `node scripts/test-macos-update-rejection.mjs /path/to/development.dmg` on a Mac to check that a genuine signed manifest cannot install an ad-hoc release. The test verifies the original bundle seal, main binary and native window after rejection. It uses a unique directory in Applications on a hosted runner and a temporary directory on a developer machine.

Linux packaging also needs the Ubuntu/Debian `fonts-noto-cjk` package or an extracted package tree in `BING_NOTO_CJK_ROOT`. The AppImage carries the regular font collection and its license so Chinese archive titles, Japanese and Korean work on a minimal desktop.

On a graphical host, run `node scripts/test-preview-memory.mjs` to measure decoded image memory while closing three real previews. It instruments a disposable copy of the native module and saves the native counters under `target/nsh-validation`.

On macOS and Linux the periodic job uses wall-clock calendar events. A short headless check compares the configured interval with the start of the last successful run. Missed minutes produce one wallpaper change after wake. The hosted-runner test in `scripts/test-unix-task-registration.sh` registers daily and periodic jobs with the operating system, triggers their packaged entry points, and removes the test registrations.

For CI distribution builds, dispatch **NSH native desktop acceptance** on the matching version tag with **distribution** enabled. Configure the repository variable `BING_WALLPAPER_UPDATE_PUBLIC_KEY` for all three platforms. In the protected `desktop-release` environment provide `MACOS_CERTIFICATE_BASE64` (Developer ID Application PKCS#12), `MACOS_CERTIFICATE_PASSWORD` and `MACOS_NOTARY_KEY` (App Store Connect private key) as secrets. Set the environment variables `MACOS_DEVELOPER_ID`, `MACOS_NOTARY_KEY_ID` and `MACOS_NOTARY_ISSUER`. The publishing workflow takes the resulting acceptance run ID and redistributes those tested artifacts.

Once the maintainer has approved both release pages, `node scripts/publish-desktop-assets.mjs ARTIFACT_DIR VERSION PRIVATE_KEY_FILE` verifies the Windows signature before its first network request. Local runs use Authenticode on Windows or `osslsigncode verify` on macOS and Linux. The protected workflow uses the digest from its Windows verification job. The script then uploads the same three files, verifies anonymous downloads, and publishes the signed manifest last. It reads `GH_TOKEN` and `GITCODE_TOKEN` on the publishing machine.
