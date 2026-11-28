#!/usr/bin/env bash
# Run with a private session bus and X server on a disposable Actions runner.
set -euo pipefail
[[ "${RUNNER_ENVIRONMENT:-}" == github-hosted && -n "$DISPLAY" ]]
image=$(realpath "$1")
work=$(mktemp -d "$RUNNER_TEMP/packaged-plasma.XXXXXX")
evidence="$RUNNER_TEMP/plasma-evidence"
export XDG_RUNTIME_DIR="$work/runtime" XDG_CONFIG_HOME="$work/config"
export XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache"
export XDG_CURRENT_DESKTOP=KDE KDE_SESSION_VERSION=5 QT_QUICK_BACKEND=software
export LIBGL_ALWAYS_SOFTWARE=1 APPIMAGE_EXTRACT_AND_RUN=1
mkdir -m 700 "$XDG_RUNTIME_DIR"
profile="$XDG_CONFIG_HOME/BingWallpaperLib"
mkdir -p "$profile" "$evidence"
dbus-update-activation-environment DISPLAY XDG_RUNTIME_DIR XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_CURRENT_DESKTOP KDE_SESSION_VERSION
kwin_x11 --replace >"$evidence/kwin.log" 2>&1 &
kwin=$!
plasmashell >"$evidence/plasma.log" 2>&1 &
shell=$!
trap 'kill "$shell" "$kwin" 2>/dev/null || true' EXIT
plasma() {
  gdbus call --session --dest org.kde.plasmashell --object-path /PlasmaShell --method org.kde.PlasmaShell.evaluateScript "$1"
}
for i in {1..60}; do
  if plasma 'if (desktopsForActivity(currentActivity()).length) print("READY")' 2>/dev/null | grep -q READY; then break; fi
  sleep 1
done
convert -size 1280x800 xc:'#202020' "$work/baseline.png"
plasma "desktopsForActivity(currentActivity()).forEach(d=>{ d.wallpaperPlugin='org.kde.image'; d.currentConfigGroup=['Wallpaper','org.kde.image','General']; d.writeConfig('Image','file://$work/baseline.png'); });"
current='desktopsForActivity(currentActivity()).forEach(d=>{d.currentConfigGroup=["Wallpaper","org.kde.image","General"]; print(d.readConfig("Image"));});'
plasma "$current" | grep -F 'baseline.png'
sleep 4
import -window root -silent "$evidence/before.png"
printf '%s\n' '{"auto_wallpaper_enabled":true,"auto_wallpaper_hour":0,"auto_wallpaper_minute":0,"download_resolution":"one_k"}' >"$profile/settings.json"
"$image" --daily-wallpaper
observed=$(plasma "$current")
[[ "$observed" == *BingWallpaperLib/Wallpapers/*.jpg* ]]
jq -e '.last_auto_wallpaper_date != null' "$profile/settings.json" >/dev/null
sleep 4
import -window root -silent "$evidence/after.png"
/usr/bin/python3 - "$evidence" "$XDG_DATA_HOME" <<'PY'
import pathlib, sys
from PIL import Image, ImageChops
folder, data = map(pathlib.Path, sys.argv[1:])
photos = list((data / "BingWallpaperLib/Wallpapers").glob("*.jpg"))
assert len(photos) == 1, photos
with Image.open(photos[0]) as photo:
    photo.load()
    print("Applied image:", photo.size)
before, after = [Image.open(folder / name).convert("RGB") for name in ("before.png", "after.png")]
diff = ImageChops.difference(before, after).resize((160,100))
changed = sum(max(pixel) > 20 for pixel in diff.getdata()) / 16000
assert changed > 0.20, f"Desktop pixels changed only {changed:.1%}"
print(f"PASS · packaged AppImage changed {changed:.1%} of the Plasma desktop")
PY
