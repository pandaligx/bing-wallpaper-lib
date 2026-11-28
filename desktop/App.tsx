import { computed, effect, Image, mount, Scroll, signal, Text, View, windowSize } from "nsh";
import type { ListRef } from "nsh";
import { Dialog, MenuButton, Progress } from "nsh/base";
import { Application, currentWindow, Tray, Window } from "nsh/desktop";
import { Library } from "wallpaper-native";
import {
  active,
  checking,
  checkUpdate,
  downloadingUpdate,
  flushHistory,
  entries,
  installUpdate,
  load,
  month,
  page,
  photos,
  pollTransfers,
  refresh,
  restoreHistory,
  selected,
  source,
  transfers,
  update,
} from "./src/library";
import {
  applied,
  canApply,
  configure,
  loadPreferences,
  preferences,
  run,
  smoothWheel,
  startup,
  status,
  isError,
  notify,
  reportError,
  retryNotice,
  t,
} from "./src/preferences";
import { Icon } from "./src/ui/Icon";
import { Action } from "./src/ui/Action";
import { Gallery, Preview } from "./src/ui/Gallery";
import { LocalGallery } from "./src/ui/LocalGallery";
import { DownloadCenter } from "./src/ui/DownloadCenter";
import { languages, Region, Settings } from "./src/ui/Settings";

const monthNames =
  "January February March April May June July August September October November December".split(" ");
const destinations = [
  { key: "home", icon: "house", label: "Home" },
  { key: "favorites", icon: "heart", label: "Favorites" },
  { key: "local", icon: "images", label: "Downloaded wallpapers" },
  { key: "downloads", icon: "arrow-down-to-line", label: "Download center" },
  { key: "settings", icon: "settings-2", label: "Settings" },
] as const;
const confirmingExit = signal(false);
let requestWindowClose = () => confirmingExit.set(true);
function UpdateProgress(props: { bytes?: number }) {
  const row = computed(() => transfers().find((tr) => tr.update && tr.state === "active"));
  const total = () => row()?.total || props.bytes || 0;
  const received = () => row()?.received || 0;
  const speed = () => row()?.speed || 0;
  const remaining = () =>
    speed() && total() ? Math.ceil(Math.max(0, total() - received()) / speed()) + " s" : "…";
  return (
    <View class="gap-1">
      <Progress
        value={total() ? (100 * received()) / total() : 0}
        indeterminate={!row() || !total()}
        accessibilityLabel={t("Download progress")}
        trackClass="h-2 rounded bg-muted"
        indicatorClass="h-2 rounded bg-primary"
      />
      <Text class="text-xs text-muted-foreground">
        {(received() / 1048576).toFixed(1)} / {(total() / 1048576).toFixed(1)} MiB
        {" · "}
        {t("Speed")} {Math.round(speed() / 1024)} KiB/s
        {" · "}
        {t("Remaining")} {remaining()}
      </Text>
    </View>
  );
}

type Route = { key: ReturnType<typeof page>; month: string };
const history: Route[] = [{ key: "home", month: "" }];
const cursor = signal(0),
  latest = signal(0);

function App() {
  const win = currentWindow();
  let galleryList: ListRef | undefined;
  const inGallery = () => page() === "home" || page() === "month" || page() === "favorites";
  const temporaryTray = signal(false);
  const show = () => {
    win.show();
    win.focus();
    temporaryTray.set(false);
  };
  const canHide = () => !!preferences()?.background_resident_enabled && Library.trayAvailable();
  const hide = () => {
    Library.trimImages();
    win.hide();
  };
  requestWindowClose = () => {
    if (canHide()) hide();
    else confirmingExit.set(true);
  };
  const drawer = signal(false),
    sidebarCollapsed = signal(false),
    expanded = signal<string[]>([]),
    ready = signal(false);
  const index = computed(() => {
    const byYear = new Map<string, string[]>();
    for (const p of photos()) {
      const year = p.date.slice(0, 4),
        ym = p.date.slice(0, 7);
      let months = byYear.get(year);
      if (!months) {
        months = [];
        byYear.set(year, months);
      }
      if (months[months.length - 1] !== ym) months.push(ym);
    }
    return [...byYear.entries()];
  });
  function visit(route: Route) {
    month.set(route.month);
    page.set(route.key);
    drawer.set(false);
  }
  function go(key: Route["key"], ym = month()) {
    if (key === page() && (key !== "month" || ym === month())) {
      drawer.set(false);
      return;
    }
    history.splice(cursor() + 1, history.length, { key, month: ym });
    cursor.set(history.length - 1);
    latest.set(cursor());
    visit(history[cursor()]);
  }
  function travel(delta: number) {
    const next = cursor() + delta;
    if (next < 0 || next >= history.length) return;
    cursor.set(next);
    visit(history[next]);
  }
  Application.on("reopen", show);
  run(async () => canApply.set(await Library.canApplyWallpaper()));
  Application.on("quit", flushHistory);
  effect(() => {
    const settings = preferences();
    if (
      !settings ||
      (!settings.background_resident_enabled && !temporaryTray()) ||
      !Library.trayAvailable()
    ) {
      Tray.set(null);
      return;
    }
    Tray.set({
      tooltip: t("Wallpaper library"),
      onClick: show,
      menu: [
        { label: t("Open main window"), onSelect: show },
        {
          label: t("Startup"),
          checked: settings.startup_enabled,
          disabled: settings.periodic_task_enabled,
          onSelect: () => run(() => startup(!settings.startup_enabled)),
        },
        {
          label: t("Run in background / show tray icon"),
          checked: settings.background_resident_enabled,
          onSelect: () => {
            show();
            run(() => configure({ background_resident_enabled: !settings.background_resident_enabled }));
          },
        },
        {
          label: t("Change wallpaper daily"),
          checked: settings.auto_wallpaper_enabled,
          onSelect: () =>
            run(() => configure({ auto_wallpaper_enabled: !settings.auto_wallpaper_enabled }, true)),
        },
        { label: t("Change once now"), onSelect: () => run(() => Library.changeNow()) },
        { type: "separator" },
        { label: t("Exit"), onSelect: () => Application.quit() },
      ],
    });
  });
  effect(() => {
    if (!active().length && !downloadingUpdate()) return;
    pollTransfers();
    const timer = setInterval(pollTransfers, 250);
    return () => clearInterval(timer);
  });
  run(async () => {
    await load();
    ready.set(true);
    run(restoreHistory);
    if (photos()[0]) expanded.set([photos()[0].date.slice(0, 4)]);
    if (photos().length)
      setTimeout(() => {
        Library.windowReady();
        run(() => Library.repairTasks());
        run(async () => {
          const error = await Library.lastUpdateError();
          if (error) reportError(error);
        });
      }, 0);
    if (source()) {
      void refresh();
      void checkUpdate(false, true);
    }
  });
  run(async () => {
    await loadPreferences();
    if (Library.backgroundLaunch() && canHide()) win.hide();
  });
  let dailyBusy = false;
  let lastDaily: string | null | undefined;
  let configuredMarker: string | undefined;
  effect(() => {
    const settings = preferences();
    if (!settings?.auto_wallpaper_enabled) return;
    const configured = settings.last_auto_wallpaper_date || "";
    if (configuredMarker !== configured) {
      lastDaily = configured;
      configuredMarker = configured;
    }
    const tick = async () => {
      if (dailyBusy) return;
      dailyBusy = true;
      try {
        const marker = await Library.tickDaily();
        if (marker && marker !== lastDaily && settings === preferences()) {
          lastDaily = marker;
          notify(t("Applied"));
          if (preferences()?.auto_wallpaper_exit_after_done) Application.quit();
        }
      } finally {
        dailyBusy = false;
      }
    };
    run(tick);
    const timer = setInterval(() => run(tick), 60_000);
    return () => clearInterval(timer);
  });
  effect(() => {
    if (!ready() || !source()) return;
    const timer = setInterval(() => void refresh(), 30 * 60 * 1000);
    return () => clearInterval(timer);
  });
  const narrow = () => windowSize().width < 800;
  const expandedRail = () => (narrow() ? drawer() : !sidebarCollapsed());
  const sidebarWidth = () => (expandedRail() ? 166 : 46);
  const brand = Library.brandIcon();
  const menuClass =
    "h-7 w-7 items-center justify-center rounded-lg hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring";
  const title = () =>
    page() === "month"
      ? month()
      : t(
          page() === "home"
            ? "Wallpaper library"
            : destinations.find((d) => d.key === page())?.label || "Home",
        );
  return (
    <View class="flex-1 bg-background text-xs text-foreground">
      <View
        class="flex-row items-center gap-1 px-2 bg-sidebar"
        style={{ height: 38, paddingRight: win.titleBar().right }}
      >
        <Action
          icon="chevron-left"
          id="nav-back"
          label={t("Back")}
          disabled={cursor() === 0}
          press={() => travel(-1)}
        />
        <Action
          icon="chevron-right"
          id="nav-forward"
          label={t("Forward")}
          disabled={cursor() >= latest()}
          press={() => travel(1)}
        />
        <Action
          icon="panel-left"
          id="navigation"
          label={t("Navigation")}
          press={() => (narrow() ? drawer.set(!drawer()) : sidebarCollapsed.set(!sidebarCollapsed()))}
        />
        <MenuButton
          testID="toolbar-theme"
          class={menuClass}
          accessibilityLabel={t("Theme")}
          items={[
            ...(["system", "light", "dark"] as const).map((id) => ({
              id,
              label: t(id === "system" ? "System" : id === "light" ? "Light" : "Dark"),
              checked: preferences()?.theme_preference === id,
            })),
            ...["Blue", "Purple", "Green"].map((id) => ({
              id,
              label: t(id),
              checked: (preferences()?.accent_color || "Blue") === id,
            })),
          ]}
          onSelect={({ id }) =>
            run(() =>
              configure(
                ["Blue", "Purple", "Green"].includes(id)
                  ? { accent_color: id }
                  : { theme_preference: id as "light" | "dark" | "system" },
              ),
            )
          }
        >
          <Icon name="palette" />
        </MenuButton>
        <MenuButton
          testID="toolbar-language"
          class={menuClass}
          accessibilityLabel={t("Language")}
          items={languages.map(([id, label]) => ({
            id,
            label: t(label),
            checked: preferences()?.language === id,
          }))}
          onSelect={({ id }) => run(() => configure({ language: id }))}
        >
          <Icon name="globe" />
        </MenuButton>
        <View class="flex-1 h-full" desktop={{ windowDrag: true }} />
      </View>
      <View class="flex-1 flex-row">
        <If when={sidebarWidth() > 0}>
          <View class="border-r border-sidebar-border bg-sidebar" style={{ width: sidebarWidth() }}>
            <View class="flex-row items-center gap-2 px-2 py-2">
              <Image
                src={brand}
                accessibilityLabel={t("Bing Wallpaper Library")}
                style={{ width: 30, height: 30, objectFit: "contain" }}
              />
              <If when={expandedRail()}>
                <Text class="text-xs font-bold text-foreground shrink line-clamp-1">
                  {t("Bing Wallpaper Library")}
                </Text>
              </If>
            </View>
            <View class="gap-1 px-2 py-1">
              <Each of={destinations.slice(0, 4)}>
                {(entry) => (
                  <Action
                    row={expandedRail()}
                    icon={entry().icon}
                    id={"nav-" + entry().key}
                    primary={page() === entry().key}
                    label={t(entry().label)}
                    shortLabel={entry().key === "local" ? t("Local photos") : undefined}
                    press={() => go(entry().key)}
                  />
                )}
              </Each>
            </View>
            <If when={expandedRail()}>
              <Text class="border-t border-sidebar-border px-3 pt-2 pb-1 text-xs font-semibold text-muted-foreground">
                {t("Archive")}
              </Text>
              <Scroll smoothWheel={smoothWheel()} testID="sidebar" class="flex-1 px-2">
                <Each of={index()}>
                  {(group) => (
                    <View>
                      <Action
                        row
                        icon={expanded().includes(group()[0]) ? "chevron-down" : "chevron-right"}
                        id={"year-" + group()[0]}
                        label={group()[0]}
                        press={() => {
                          const y = group()[0];
                          expanded.update((years) =>
                            years.includes(y) ? years.filter((v) => v !== y) : [...years, y],
                          );
                        }}
                      />
                      <If when={expanded().includes(group()[0])}>
                        <View class="ml-4 border-l border-sidebar-border pl-1">
                          <Each of={group()[1]}>
                            {(ym) => (
                              <Action
                                row
                                id={"month-" + ym()}
                                label={ym()}
                                shortLabel={t(monthNames[Number(ym().slice(5)) - 1])}
                                primary={page() === "month" && month() === ym()}
                                press={() => {
                                  go("month", ym());
                                }}
                              />
                            )}
                          </Each>
                        </View>
                      </If>
                    </View>
                  )}
                </Each>
              </Scroll>
            </If>
            <If when={!expandedRail()}>
              <View class="flex-1" />
            </If>
            <View class="border-t border-sidebar-border p-2">
              <Action
                row={expandedRail()}
                icon="settings-2"
                id="nav-settings"
                primary={page() === "settings"}
                label={t("Settings")}
                press={() => go("settings")}
              />
            </View>
          </View>
        </If>
        <View class="flex-1 gap-2 p-3">
          <View class="flex-row items-center gap-2">
            <Text testID="page-title" class="text-base font-semibold text-foreground">
              {title()}
            </Text>
            <If when={inGallery()}>
              <Text class="flex-1 text-xs text-muted-foreground">
                {entries().length} {t("Wallpapers")}
              </Text>
              <Action
                icon="arrow-up"
                label={t("Back to top")}
                press={() => galleryList?.scrollToOffset({ offset: 0, animated: true })}
              />
            </If>
          </View>
          <If when={!ready()}>
            <Text class="text-xs">{t("Loading wallpaper list...")}</Text>
          </If>
          <If when={ready() && !source() && page() !== "settings"}>
            <Region />
          </If>
          <If when={inGallery()}>
            <Gallery
              sidebarWidth={sidebarWidth()}
              listRef={(node) => {
                galleryList = node;
              }}
            />
          </If>
          <If when={page() === "local"}>
            <LocalGallery sidebarWidth={sidebarWidth()} />
          </If>
          <If when={page() === "downloads"}>
            <DownloadCenter />
          </If>
          <If when={page() === "settings" && !!preferences()}>
            <Settings />
          </If>
          <If when={!!status()}>
            <View
              class="flex-row items-center gap-2 border-t border-border py-1"
              accessibilityLiveRegion={isError() ? "assertive" : "polite"}
            >
              <Icon
                name={isError() ? "circle-alert" : "circle-check"}
                ink={isError() ? "destructive" : "muted-foreground"}
              />
              <Text
                testID={applied() && status() === t("Applied") ? "applied" : "status"}
                class="flex-1 text-xs text-foreground"
              >
                {status()}
              </Text>
              <If when={retryNotice()}>
                {(retry) => (
                  <Action
                    icon="rotate-ccw"
                    id="retry-status"
                    label={t("Retry")}
                    press={() => {
                      const action = retry();
                      notify("");
                      action();
                    }}
                  />
                )}
              </If>
              <Action id="dismiss-status" icon="x" label={t("Close")} press={() => notify("")} />
            </View>
          </If>
        </View>
      </View>
      <If when={selected()}>{(photo) => <Preview photo={photo()} />}</If>
      <If when={update()}>
        {(item) => (
          <Dialog
            open
            class="m-auto rounded-lg bg-card"
            onOpenChange={({ open }) => {
              if (!open) update.set(null);
            }}
          >
            <View class="gap-3 p-4">
              <Text class="text-sm font-semibold text-foreground">
                {t("Update available")} · {item().version}
              </Text>
              <Text class="text-xs text-muted-foreground">
                {item().path ||
                  [source(), item().minimum, ((item().bytes || 0) / 1048576).toFixed(1) + " MiB"].join(" · ")}
              </Text>
              <If when={downloadingUpdate()}>
                <UpdateProgress bytes={item().bytes} />
              </If>
              <View class="flex-row flex-wrap gap-2">
                <Action label={t("Later")} press={() => update.set(null)} />
                <Action
                  icon="external-link"
                  label={t("View release notes")}
                  press={() => run(() => Library.openLocation("releases"))}
                />
                <Action
                  primary
                  disabled={checking()}
                  label={t(item().state === "staged" ? "Update now" : "Download")}
                  press={() => {
                    if (item().state === "staged") void installUpdate();
                    else void checkUpdate(true);
                  }}
                />
              </View>
            </View>
          </Dialog>
        )}
      </If>
      <If when={confirmingExit()}>
        <Dialog
          open
          class="m-auto rounded-xl bg-card"
          role="alertdialog"
          style={{ width: Math.min(360, windowSize().width - 32), alignSelf: "center" }}
          backdropStyle={{ backgroundColor: "rgba(32,36,42,0.24)", backdropFilter: "blur(8px)" }}
          onOpenChange={({ open }) => {
            if (!open) confirmingExit.set(false);
          }}
        >
          <View testID="exit-prompt" class="gap-3 p-4">
            <Text class="text-xs font-semibold text-foreground">{t("Exit app prompt")}</Text>
            <View class="flex-row gap-2">
              <Action id="cancel-exit" label={t("Cancel")} press={() => confirmingExit.set(false)} />
              <If when={Library.trayAvailable()}>
                <Action
                  label={t("Minimize to tray")}
                  press={() => {
                    confirmingExit.set(false);
                    temporaryTray.set(true);
                    hide();
                  }}
                />
              </If>
              <Action primary label={t("Exit application")} press={() => Application.quit()} />
            </View>
          </View>
        </Dialog>
      </If>
    </View>
  );
}
mount(() => (
  <Window
    id="main"
    title="必应每日壁纸库"
    titleBar={{ style: "hidden", height: 38 }}
    closeOnPress={false}
    onClosePress={() => requestWindowClose()}
  >
    {App}
  </Window>
));
