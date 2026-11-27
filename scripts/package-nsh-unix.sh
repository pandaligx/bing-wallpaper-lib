#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/desktop"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$root/target/nsh-cargo"}
out="$root/target/nsh-release"
mkdir -p "$out"
version=$(node -p "require('./nsh.json').version")
manifest=ext/wallpaper-native/Cargo.toml
case "$1" in
  macos)
    export MACOSX_DEPLOYMENT_TARGET=14.0
    nsh package --target macos --arch universal
    app=".nsh/out/BingWallpaperLib.app"
    binary=$(/usr/libexec/PlistBuddy -c 'Print CFBundleExecutable' "$app/Contents/Info.plist")
    lipo "$app/Contents/MacOS/$binary" -verify_arch arm64 x86_64
    mkdir -p "$app/Contents/Helpers"
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
      cargo build --locked --release --target "$target" --manifest-path "$manifest" --bin bing-wallpaper-task
    done
    helper="$app/Contents/Helpers/bing-wallpaper-task"
    lipo -create "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/bing-wallpaper-task" "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/bing-wallpaper-task" -output "$helper"
    lipo "$helper" -verify_arch arm64 x86_64
    # Development packages use local ad-hoc signatures. Release signing follows notarization setup.
    codesign --force --sign - "$helper"
    codesign --force --sign - "$app"
    codesign --verify --deep --strict "$app"
    hdiutil create -ov -volname BingWallpaperLib -srcfolder "$app" -format UDZO "$out/BingWallpaperLib-v$version-macos-universal.dmg"
    ;;
  linux)
    cargo build --locked --release --manifest-path "$manifest" --bin bing-wallpaper-task
    export BING_WALLPAPER_TASK="$CARGO_TARGET_DIR/release/bing-wallpaper-task"
    export BING_APPIMAGETOOL=${NSH_APPIMAGETOOL:-$(command -v appimagetool)}
    NSH_APPIMAGETOOL="$root/scripts/nsh-appimagetool.sh" nsh package --target linux --format appimage
    cp .nsh/out/BingWallpaperLib-x86_64.AppImage "$out/BingWallpaperLib-v$version-linux-x86_64.AppImage"
    ;;
  *) echo "Expected macos or linux" >&2; exit 2 ;;
esac
