#!/usr/bin/env bash
# NSH 0.7.2 invokes appimagetool --no-appstream APPDIR IMAGE after assembling APPDIR.
set -euo pipefail
[[ $# -eq 3 && "$1" == "--no-appstream" && -f "$2/AppRun" ]]
appdir=$2
install -Dm755 "$BING_WALLPAPER_TASK" "$appdir/usr/libexec/bing-wallpaper-task"
cp "$appdir/AppRun" "$appdir/AppRun.nsh"
cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
dir=$(dirname "$(readlink -f "$0")")
case "${1:-}" in
  --daily-wallpaper|--scheduled-wallpaper) exec "$dir/usr/libexec/bing-wallpaper-task" "$@" ;;
  *) exec "$dir/AppRun.nsh" "$@" ;;
esac
RUN
exec "$BING_APPIMAGETOOL" "$@"
