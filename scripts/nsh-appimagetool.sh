#!/usr/bin/env bash
# NSH 0.7.2 invokes appimagetool --no-appstream APPDIR IMAGE after assembling APPDIR.
set -euo pipefail
[[ $# -eq 3 && "$1" == "--no-appstream" && -f "$2/AppRun" ]]
appdir=$2
install -Dm755 "$BING_WALLPAPER_TASK" "$appdir/usr/libexec/bing-wallpaper-task"
# Even the English interface displays Chinese titles from the Bing archive.
font_root=${BING_NOTO_CJK_ROOT:-/}
resources="$appdir/usr/share/bing-wallpaper"
install -Dm644 "$font_root/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc" "$resources/fonts/NotoSansCJK-Regular.ttc"
install -Dm644 "$font_root/usr/share/doc/fonts-noto-cjk/copyright" "$resources/licenses/fonts-noto-cjk"
cat > "$resources/fonts.conf" <<'FONTCONFIG'
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <include ignore_missing="yes">/etc/fonts/fonts.conf</include>
  <dir prefix="relative">fonts</dir>
</fontconfig>
FONTCONFIG
cp "$appdir/AppRun" "$appdir/AppRun.nsh"
cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
dir=$(dirname "$(readlink -f "$0")")
case "${1:-}" in
  --daily-wallpaper|--scheduled-wallpaper|--install-wallpaper-update) exec "$dir/usr/libexec/bing-wallpaper-task" "$@" ;;
  *) export FONTCONFIG_FILE="$dir/usr/share/bing-wallpaper/fonts.conf"
     exec "$dir/AppRun.nsh" "$@" ;;
esac
RUN
exec "$BING_APPIMAGETOOL" "$@"
