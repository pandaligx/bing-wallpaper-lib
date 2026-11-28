import { appInfo, Clipboard, Scroll, signal, View, windowSize } from "nsh";
import { Text } from "./Text";
import { Dialog, Input, Slider, Toolbar } from "nsh/base";
import { Library } from "wallpaper-native";
import { checking, checkUpdate, load, refresh, setSource, source, sourcePreference } from "../library";
import { configure, preferences, uiFontSize, run, startup, notify, reportError, t } from "../preferences";
import type { Preferences } from "../preferences";
import { Icon, type IconName } from "./Icon";
import { Action } from "./Action";
import { ChoiceField, FieldLabel, Toggle } from "./WallpaperOptions";

export function Region() {
  return (
    <View class="gap-2">
      <Text class="text-xs font-semibold text-foreground">
        {t("Region")}
        {source() ? " · " + source() : ""}
      </Text>
      <View class="flex-row flex-wrap gap-2">
        <Action
          label={t("Follow system")}
          primary={sourcePreference() === "auto"}
          press={() => setSource("auto")}
        />
        <Action
          id="region-cn"
          label="GitCode · CN"
          primary={sourcePreference() === "cn"}
          press={() => setSource("cn")}
        />
        <Action
          id="region-other"
          label="GitHub"
          primary={sourcePreference() === "international"}
          press={() => setSource("international")}
        />
      </View>
    </View>
  );
}
export const languages = [
  ["system", "Follow system"],
  ["english", "English"],
  ["simplified_chinese", "简体中文"],
  ["japanese", "日本語"],
  ["korean", "한국어"],
  ["russian", "Русский"],
  ["french", "Français"],
];
const sources = [
  ["latest", "Latest daily wallpaper"],
  ["random_all", "Random from all history"],
  ["random_favorites", "Random from favorites"],
];
function choices(pairs: string[][]) {
  return pairs.map(([value, title]) => ({ value, title: t(title) }));
}
function Heading(props: { label: string; icon: IconName }) {
  return (
    <View class="flex-row items-center gap-2 pb-1 pt-2">
      <Icon name={props.icon} ink="muted-foreground" />
      <Text class="text-xs font-semibold text-foreground">{props.label}</Text>
    </View>
  );
}
const inputClass = "h-7 min-w-20 rounded border border-input bg-card px-2 text-xs text-foreground";

export function Settings() {
  const s = () => preferences()!;
  const diagnostic = signal<string | null>(null);
  const path = signal(""),
    time = signal<string | null>(null),
    interval = signal<string | null>(null);
  const displays = signal<{ id: string; label: string; width?: number; height?: number }[]>([]);
  function getDisplays() {
    try {
      displays.set(JSON.parse(Library.monitors()));
    } catch (e) {
      reportError(e);
    }
  }
  getDisplays();
  run(async () => path.set(await Library.downloadFolder()));
  const clock = () =>
    String(s().auto_wallpaper_hour).padStart(2, "0") +
    ":" +
    String(s().auto_wallpaper_minute).padStart(2, "0");
  const save = (patch: Partial<Preferences>, scheduled = false) => run(() => configure(patch, scheduled));
  const monitorOptions = () => {
    const options = [
      { value: "", title: t("Sync all displays"), disabled: false },
      ...displays().map((m, index) => ({
        value: m.id,
        title:
          m.width && m.height ? t("Display") + " " + (index + 1) + " · " + m.width + "×" + m.height : m.label,
        disabled: false,
      })),
    ];
    const current = s().wallpaper_target.monitor_id;
    if (current && !options.some((m) => m.value === current))
      options.push({ value: current, title: t("Disconnected"), disabled: true });
    return options;
  };
  return (
    <Scroll smoothWheel={false} testID="settings-content" class="flex-1">
      <View class="gap-2 pb-4 pr-2" style={{ width: "100%", maxWidth: 650 }}>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="palette" label={t("Appearance")} />
          <View class="gap-1">
            <ChoiceField
              id="theme"
              label={t("Theme")}
              value={s().theme_preference}
              items={choices([
                ["system", "System"],
                ["light", "Light"],
                ["dark", "Dark"],
              ])}
              choose={(v) => save({ theme_preference: v as Preferences["theme_preference"] })}
            />
            <ChoiceField
              id="language"
              label={t("Language")}
              value={s().language}
              items={choices(languages)}
              choose={(v) => save({ language: v })}
            />
          </View>
          <View class="flex-row items-center gap-2">
            <FieldLabel label={t("Font size")} />
            <Slider
              testID="font-size"
              min={10}
              max={20}
              step={1}
              value={uiFontSize()}
              accessibilityLabel={t("Font size")}
              style={{ width: 120 }}
              trackClass="h-6 justify-center"
              indicatorClass="h-1 rounded-full bg-muted-foreground"
              rangeClass="rounded-full bg-primary"
              thumbClass="size-3 rounded-full bg-primary"
              onChange={({ value }) => {
                if (typeof value === "number") uiFontSize.set(value);
              }}
              onChangeEnd={({ value }) => {
                if (typeof value === "number") save({ ui_font_size: value });
              }}
            />
            <Text class="text-xs text-muted-foreground">{uiFontSize()}</Text>
          </View>
        </View>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="hard-drive" label={t("Storage")} />
          <View class="flex-row items-center gap-1">
            <Input
              style={{ flex: 1, fontSize: uiFontSize() }}
              testID="folder-path"
              accessibilityLabel={t("Wallpaper download folder")}
              class={inputClass}
              value={path()}
              onChange={({ value }) => path.set(value)}
            />
            <Toolbar accessibilityLabel={t("Wallpaper download folder")} class="gap-1">
              <Action
                icon="folder-search"
                label={t("Choose and save")}
                press={() =>
                  run(async () => {
                    const result = await Library.chooseFolder();
                    if (result) {
                      await configure({ download_dir: result });
                      path.set(result);
                    }
                  })
                }
              />
              <Action
                icon="save"
                label={t("Save")}
                press={() =>
                  run(async () => {
                    await configure({ download_dir: path().trim() || null });
                    path.set(await Library.downloadFolder());
                  })
                }
              />
              <Action
                icon="rotate-ccw"
                id="default-folder"
                label={t("Default")}
                press={() =>
                  run(async () => {
                    await configure({ download_dir: null });
                    path.set(await Library.downloadFolder());
                  })
                }
              />
              <Action
                icon="folder-open"
                label={t("Open folder")}
                press={() => run(() => Library.openLocation("folder"))}
              />
            </Toolbar>
          </View>
          <ChoiceField
            id="resolution"
            label={t("Global resolution")}
            value={s().download_resolution}
            items={choices([
              ["original", t("Original") + " UHD"],
              ["four_k", "4K · 3840×2160"],
              ["two_k", "2K · 2560×1440"],
              ["one_k", "1K · 1920×1080"],
            ])}
            choose={(v) => save({ download_resolution: v })}
          />
        </View>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="monitor" label={t("Display")} />
          <View class="flex-row flex-wrap items-center gap-2">
            <ChoiceField
              id="monitor-target"
              hideLabel
              label={t("Multi-monitor wallpaper")}
              items={monitorOptions()}
              value={s().wallpaper_target.monitor_id ?? ""}
              choose={(v) =>
                save({ wallpaper_target: v ? { type: "monitor", monitor_id: v } : { type: "all" } })
              }
            />
            <Action icon="refresh-cw" label={t("Refresh")} press={getDisplays} />
          </View>
        </View>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="clock-3" label={t("Automatic wallpaper")} />
          <Toggle
            label={t("Run in background / show tray icon")}
            value={s().background_resident_enabled}
            change={(v) => save({ background_resident_enabled: v })}
          />
          <Toggle
            label={t("Startup")}
            value={s().startup_enabled}
            disabled={s().periodic_task_enabled}
            change={(v) => run(() => startup(v))}
          />
          <Toggle
            id="daily-enabled"
            label={t("Change wallpaper daily")}
            value={s().auto_wallpaper_enabled}
            change={(v) => save({ auto_wallpaper_enabled: v }, true)}
          />
          <ChoiceField
            label={t("Wallpaper source")}
            items={choices(sources)}
            value={s().auto_wallpaper_source}
            choose={(v) => save({ auto_wallpaper_source: v }, true)}
          />
          <View class="flex-row flex-wrap items-center gap-2">
            <FieldLabel label={t("Execution time")} />
            <Input
              style={{ width: 84, fontSize: uiFontSize() }}
              class={inputClass}
              value={time() ?? clock()}
              accessibilityLabel={t("Execution time")}
              onChange={({ value }) => time.set(value)}
            />
            <Action
              icon="save"
              label={t("Save")}
              disabled={!/^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(time() ?? "")}
              press={() => {
                const [hour, minute] = (time() ?? clock()).split(":").map(Number);
                save({ auto_wallpaper_hour: hour, auto_wallpaper_minute: minute }, true);
              }}
            />
            <Action
              icon="play"
              label={t("Change once now")}
              press={() =>
                run(async () => {
                  await Library.changeNow();
                  notify(t("Applied"));
                })
              }
            />
          </View>
          <If when={time() !== null && !/^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(time()!)}>
            <Text class="text-xs text-destructive">{t("Use HH:mm")}</Text>
          </If>
          <Toggle
            label={t("Exit after automatic wallpaper")}
            value={s().auto_wallpaper_exit_after_done}
            change={(v) => save({ auto_wallpaper_exit_after_done: v })}
          />
          <Toggle
            id="periodic-enabled"
            label={t("Enable periodic task scheduler")}
            value={s().periodic_task_enabled}
            change={(v) =>
              run(async () => {
                const disableStartup = v && s().startup_enabled;
                if (disableStartup) await startup(false);
                try {
                  await configure({ periodic_task_enabled: v }, true);
                } catch (e) {
                  if (disableStartup) await startup(true);
                  throw e;
                }
              })
            }
          />
          <Toggle
            label={t("Use latest Bing wallpaper for the first run each day")}
            value={s().periodic_daily_first_latest}
            change={(v) => save({ periodic_daily_first_latest: v }, true)}
          />
          <ChoiceField
            label={t("Later wallpaper source")}
            value={s().periodic_wallpaper_source}
            items={choices(sources.slice(1))}
            choose={(v) => save({ periodic_wallpaper_source: v }, true)}
          />
          <View class="flex-row flex-wrap items-center gap-2">
            <FieldLabel label={t("Repeat interval") + " (min)"} />
            <Input
              style={{ width: 84, fontSize: uiFontSize() }}
              class={inputClass}
              accessibilityLabel={t("Repeat interval")}
              value={interval() ?? String(s().periodic_interval_minutes)}
              onChange={({ value }) => interval.set(value)}
            />
            <Action
              icon="save"
              label={t("Save")}
              disabled={
                !/^\d{1,4}$/.test(interval() ?? "") || Number(interval()) < 1 || Number(interval()) > 1439
              }
              press={() => save({ periodic_interval_minutes: Number(interval()) }, true)}
            />
          </View>
          <If
            when={
              interval() !== null &&
              (!/^\d{1,4}$/.test(interval()!) || Number(interval()) < 1 || Number(interval()) > 1439)
            }
          >
            <Text class="text-xs text-destructive">{t("Enter 1–1439 minutes")}</Text>
          </If>
        </View>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="globe" label={t("Maintenance")} />
          <Region />
          <Toolbar class="gap-1" accessibilityLabel={t("Maintenance")}>
            <Action
              icon="refresh-cw"
              label={t("Refresh wallpaper list")}
              disabled={!source()}
              press={() => void refresh()}
            />
            <Action
              icon="trash-2"
              label={t("Clear wallpaper cache")}
              press={() =>
                run(async () => {
                  await Library.clearThumbnailCache();
                  await load();
                  if (source()) await refresh();
                })
              }
            />
            <Action
              icon="arrow-down-to-line"
              id="check-update"
              label={t("Check for updates")}
              disabled={!source() || checking()}
              press={() => void checkUpdate()}
            />
            <Action
              icon="external-link"
              label={t("View release notes")}
              disabled={!source()}
              press={() => run(() => Library.openLocation("releases"))}
            />
          </Toolbar>
        </View>
        <View class="gap-1 rounded-xl bg-card p-3">
          <Heading icon="info" label={t("About")} />
          <Action
            row
            icon="circle-alert"
            id="diagnostics"
            label={t("Diagnostics")}
            press={() => run(async () => diagnostic.set(await Library.diagnosticReport()))}
          />
          <If when={diagnostic() !== null}>
            <Dialog
              open
              class="m-auto rounded-xl bg-card"
              style={{ width: Math.min(530, windowSize().width - 32), alignSelf: "center" }}
              backdropStyle={{ backdropFilter: "blur(8px)", backgroundColor: "rgba(65,70,80,0.18)" }}
              onOpenChange={({ open }) => {
                if (!open) diagnostic.set(null);
              }}
            >
              <View class="gap-2 p-4">
                <View class="flex-row items-center">
                  <Text class="flex-1 text-xs font-bold text-foreground">{t("Diagnostics")}</Text>
                  <Action
                    id="close-diagnostics"
                    icon="x"
                    label={t("Close")}
                    press={() => diagnostic.set(null)}
                  />
                </View>
                <Scroll
                  class="rounded-lg bg-muted p-2"
                  style={{ height: Math.min(210, windowSize().height / 3) }}
                >
                  <Text class="text-xs text-foreground">{diagnostic()}</Text>
                </Scroll>
                <View class="flex-row gap-1">
                  <Action
                    icon="list-checks"
                    label={t("Copy log")}
                    press={() =>
                      run(async () => {
                        if ((await Clipboard.writeText(diagnostic() || "")) === "failed")
                          throw Error(t("Failed"));
                        notify(t("Copied"));
                      })
                    }
                  />
                  <Action
                    icon="folder-open"
                    label={t("Open log")}
                    press={() => run(() => Library.openLocation("log"))}
                  />
                  <Action
                    row
                    icon="external-link"
                    label={t("Create issue")}
                    press={() => run(() => Library.openLocation("issue"))}
                  />
                </View>
              </View>
            </Dialog>
          </If>
          <View class="flex-row items-center gap-2">
            <Text class="text-xs text-muted-foreground">
              {t("Version")} {Library.appVersion()} ({appInfo().build}) · © 2023-2026 小南瓜
            </Text>
            <Action
              icon="external-link"
              label={t("Project home")}
              disabled={!source()}
              press={() => run(() => Library.openLocation("project"))}
            />
          </View>
        </View>
      </View>
    </Scroll>
  );
}
