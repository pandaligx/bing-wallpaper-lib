#!/usr/bin/env bash
# Invoked only inside an ephemeral CI D-Bus/Xvfb session.
set -euo pipefail
export XDG_CURRENT_DESKTOP=KDE
export KDE_SESSION_VERSION=5
export QT_QUICK_BACKEND=software
export XDG_RUNTIME_DIR
XDG_RUNTIME_DIR=$(mktemp -d)
dbus-update-activation-environment DISPLAY XDG_RUNTIME_DIR XDG_CURRENT_DESKTOP KDE_SESSION_VERSION
kwin_x11 --replace > "$RUNNER_TEMP/kwin.log" 2>&1 &
kwin_pid=$!
plasmashell > "$RUNNER_TEMP/plasmashell.log" 2>&1 &
plasma_pid=$!
trap 'kill "$plasma_pid" "$kwin_pid" 2>/dev/null || true' EXIT
plasma_eval() {
  gdbus call --session --dest org.kde.plasmashell --object-path /PlasmaShell --method org.kde.PlasmaShell.evaluateScript "$1"
}
for n in {1..45}; do
  if plasma_eval 'if (desktopsForActivity(currentActivity()).length) print("READY")' 2>/dev/null | grep -q READY; then break; fi
  sleep 1
done
nsh test desktop/App.tsx --target linux < scripts/test-nsh-desktop.mjs
observed=$(plasma_eval 'desktopsForActivity(currentActivity()).forEach(d => { d.currentConfigGroup=["Wallpaper","org.kde.image","General"]; print(d.readConfig("Image")); })')
[[ "$observed" == *BingWallpaperLib/Wallpapers/*.jpg* ]]
