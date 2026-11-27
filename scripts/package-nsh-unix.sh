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
    identity=${MACOS_DEVELOPER_ID:--}
    signing=(--force --sign "$identity")
    if [[ "$identity" != "-" ]]; then
      : "${MACOS_NOTARY_PROFILE:?Set the notarytool keychain profile for this Developer ID build}"
      signing+=(--timestamp --options runtime)
    fi
    codesign "${signing[@]}" "$helper"
    codesign "${signing[@]}" "$app"
    codesign --verify --deep --strict "$app"
    dmg="$out/BingWallpaperLib-v$version-macos-universal.dmg"
    if [[ "$identity" != "-" ]]; then
      archive=".nsh/out/BingWallpaperLib-notary.zip"
      ditto -c -k --keepParent "$app" "$archive"
      xcrun notarytool submit "$archive" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
      xcrun stapler staple "$app"
      spctl --assess --type execute "$app"
    fi
    hdiutil create -ov -volname BingWallpaperLib -srcfolder "$app" -format UDZO "$dmg"
    if [[ "$identity" != "-" ]]; then
      codesign --force --timestamp --sign "$identity" "$dmg"
      xcrun notarytool submit "$dmg" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
      xcrun stapler staple "$dmg"
      xcrun stapler validate "$dmg"
    fi
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
