#!/usr/bin/env bash
# Run only on a fresh hosted desktop. Register through Rust, then trigger both OS jobs.
set -euo pipefail
[[ "${RUNNER_ENVIRONMENT:-}" == "github-hosted" ]]
run=$(mktemp -d "$RUNNER_TEMP/scheduler.XXXXXX")
test_bin=$(cargo test --locked --manifest-path desktop/ext/wallpaper-native/Cargo.toml --lib --no-run --message-format=json |
  jq -r 'select(.reason == "compiler-artifact" and .profile.test and .executable != null) | .executable' | tail -1)
test_name=registration::tests::register_native_desktop_tasks
system=$(uname -s)
if [[ $system == Darwin ]]; then
  app="$run/Wallpaper Scheduler.app/Contents"
  mkdir -p "$app/MacOS" "$app/Helpers"
  cp "$test_bin" "$app/MacOS/scheduler-test"
  test_bin="$app/MacOS/scheduler-test"
  stub="$app/Helpers/bing-wallpaper-task"
  real="$(pwd)/desktop/.nsh/out/BingWallpaperLib.app/Contents/Helpers/bing-wallpaper-task"
  domain="gui/$(id -u)"
else
  uid=$(id -u)
  export XDG_RUNTIME_DIR="/run/user/$uid"
  export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
  sudo loginctl enable-linger "$(id -un)"
  sudo systemctl start "user@$uid.service"
  systemctl --user is-active default.target
  stub="$run/Wallpaper Scheduler.AppImage"
  export APPIMAGE="$stub"
  images=("$(pwd)"/target/nsh-release/*.AppImage)
  real=${images[0]}
fi
[[ -x "$real" && -x "$test_bin" ]]
# Record the exact argument only after the packaged headless entry point returns successfully.
{
  echo '#!/usr/bin/env bash'
  echo 'set -eu'
  printf 'APPIMAGE_EXTRACT_AND_RUN=1 %q "$@"\n' "$real"
  printf 'printf "%%s\\n" "$1" >> %q\n' "$run/invocations"
} > "$stub"
chmod +x "$stub"
register() { BING_SCHEDULER_TEST="$1" "$test_bin" --ignored --exact "$test_name"; }
trap 'register disable' EXIT
register enable
if [[ $system == Darwin ]]; then
  path="$HOME/Library/LaunchAgents/cn.lgxng.bingwallpaperlib"
  for mode in daily periodic; do
    plutil -lint "$path.$mode.plist"
    label="$domain/cn.lgxng.bingwallpaperlib.$mode"
    launchctl print "$label" | grep -F "$stub" >/dev/null
    launchctl kickstart "$label"
  done
  [[ $(/usr/libexec/PlistBuddy -c 'Print StartCalendarInterval:Minute' "$path.daily.plist") == 43 ]]
  /usr/libexec/PlistBuddy -c 'Print StartCalendarInterval' "$path.periodic.plist" >/dev/null
else
  for mode in daily periodic; do
    unit="bing-wallpaper-$mode"
    systemctl --user is-enabled "$unit.timer"
    systemctl --user is-active "$unit.timer"
    systemctl --user cat "$unit.timer" | grep -F 'Persistent=true'
    systemctl --user start "$unit.service"
  done
fi
for i in {1..60}; do
  if [[ -f "$run/invocations" ]] && grep -Fxq -- --daily-wallpaper "$run/invocations" &&
     grep -Fxq -- --scheduled-wallpaper "$run/invocations"; then break; fi
  sleep 1
done
grep -Fx -- --daily-wallpaper "$run/invocations"
grep -Fx -- --scheduled-wallpaper "$run/invocations"
register disable
trap - EXIT
if [[ $system == Darwin ]]; then
  [[ ! -e "$path.daily.plist" && ! -e "$path.periodic.plist" ]]
else
  [[ ! -e "$HOME/.config/systemd/user/bing-wallpaper-daily.timer" &&
     ! -e "$HOME/.config/systemd/user/bing-wallpaper-periodic.timer" ]]
fi
echo "PASS · registered, triggered and removed both native desktop tasks"
