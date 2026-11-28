#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/desktop"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$root/target/nsh-cargo"}
out="$root/target/nsh-release"
mkdir -p "$out"
version=$(node -p "require('./nsh.json').version")
manifest=ext/wallpaper-native/Cargo.toml
[[ "$1" == linux ]]
cargo build --locked --release --manifest-path "$manifest" --bin bing-wallpaper-task
export BING_WALLPAPER_TASK="$CARGO_TARGET_DIR/release/bing-wallpaper-task"
export BING_APPIMAGETOOL=${NSH_APPIMAGETOOL:-$(command -v appimagetool)}
NSH_APPIMAGETOOL="$root/scripts/nsh-appimagetool.sh" nsh package --target linux --format appimage
cp .nsh/out/BingWallpaperLib-x86_64.AppImage "$out/BingWallpaperLib-v$version-linux-x86_64.AppImage"
