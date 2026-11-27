#!/usr/bin/env bash
# Run inside xvfb-run and dbus-run-session on a disposable hosted Ubuntu desktop.
set -euo pipefail
[[ "${RUNNER_ENVIRONMENT:-}" == github-hosted && -n "$DISPLAY" ]]
image=$(realpath "$1")
work=$(mktemp -d "$RUNNER_TEMP/gnome-wallpaper.XXXXXX")
export XDG_RUNTIME_DIR="$work/runtime" XDG_CONFIG_HOME="$work/config"
export XDG_DATA_HOME="$work/data" XDG_CACHE_HOME="$work/cache"
export XDG_CURRENT_DESKTOP=GNOME LIBGL_ALWAYS_SOFTWARE=1 APPIMAGE_EXTRACT_AND_RUN=1
mkdir -m 700 "$XDG_RUNTIME_DIR"
profile="$XDG_CONFIG_HOME/BingWallpaperLib"
mkdir -p "$profile" "$RUNNER_TEMP/gnome-evidence"
dbus-update-activation-environment DISPLAY XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_CURRENT_DESKTOP XDG_RUNTIME_DIR
printf '%s\n' '{"auto_wallpaper_enabled":true,"auto_wallpaper_hour":0,"auto_wallpaper_minute":0,"download_resolution":"one_k"}' > "$profile/settings.json"
gsettings set org.gnome.desktop.interface enable-animations false
gsettings set org.gnome.desktop.notifications show-banners false
gsettings set org.gnome.desktop.background picture-uri "''"
gsettings set org.gnome.desktop.background picture-uri-dark "''"
gsettings set org.gnome.desktop.background primary-color "'#202020'"
log="$RUNNER_TEMP/gnome-evidence/shell.log"
gnome-shell --nested --wayland --no-x11 --wayland-display=wallpaper-test >"$log" 2>&1 &
shell_pid=$!
trap 'kill "$shell_pid" 2>/dev/null || true' EXIT
for i in {1..60}; do
  if [[ -S "$XDG_RUNTIME_DIR/wallpaper-test" ]] && gdbus call --session --dest org.gnome.Mutter.DisplayConfig \
     --object-path /org/gnome/Mutter/DisplayConfig --method org.gnome.Mutter.DisplayConfig.GetCurrentState > "$work/monitors"; then break; fi
  kill -0 "$shell_pid" || { cat "$log"; exit 1; }
  sleep 1
done
test -s "$work/monitors"
sleep 4
captures="$RUNNER_TEMP/gnome-evidence"
import -window root -silent "$captures/before.png"
"$image" --daily-wallpaper
light=$(gsettings get org.gnome.desktop.background picture-uri)
dark=$(gsettings get org.gnome.desktop.background picture-uri-dark)
[[ "$light" == "$dark" && "$light" == *BingWallpaperLib/Wallpapers/*.jpg* ]]
jq -e '.last_auto_wallpaper_date != null' "$profile/settings.json" >/dev/null
sleep 5
import -window root -silent "$captures/after.png"
/usr/bin/python3 - "$captures" "$light" <<'PY'
import pathlib, sys
from urllib.parse import urlsplit, unquote
from PIL import Image, ImageChops
folder = pathlib.Path(sys.argv[1])
photo = pathlib.Path(unquote(urlsplit(sys.argv[2].strip("'")).path))
with Image.open(photo) as source:
    source.load()
    print("Applied image:", source.size)
before, after = [Image.open(folder / (name + ".png")).convert("RGB") for name in ("before", "after")]
assert before.size == after.size
# Nested Mutter may use an 800x600 output inside a larger Xvfb framebuffer.
# The nonblack shell surface in the initial capture excludes the inactive host margins.
bounds = before.getbbox()
assert bounds and bounds[2] - bounds[0] >= 640 and bounds[3] - bounds[1] >= 480, bounds
before, after = before.crop(bounds), after.crop(bounds)
before.save(folder / "desktop-before.png")
after.save(folder / "desktop-after.png")
diff = ImageChops.difference(before, after).resize((160, 100))
changed = sum(max(pixel) > 20 for pixel in diff.getdata()) / 16000
assert changed > 0.20, f"Compositor background did not visibly change: {changed:.1%}"
print(f"PASS · running GNOME Shell redrew {changed:.1%} of the sampled desktop after the AppImage applied the photo")
PY
