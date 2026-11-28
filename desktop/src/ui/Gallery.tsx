import { computed, effect, Image, keymap, List, signal, View, windowSize } from "nsh";
import { currentWindow } from "nsh/desktop";
import { Text } from "./Text";
import type { ListRef, ViewRef } from "nsh";
import { PhotoViewer } from "./PhotoViewer";
import { Button, Progress, Toolbar } from "nsh/base";
import {
  active,
  download,
  entries,
  favorite,
  month,
  page,
  photos,
  saved,
  selected,
  transfers,
} from "../library";
import type { Photo } from "../library";
import { canApply, t } from "../preferences";
import { Action } from "./Action";
import { releaseUnusedImages, Thumbnail } from "./Thumbnail";

const positions = new Map<string, number>();
let previewOpener: ViewRef | undefined;
function openPreview(photo: Photo, opener?: ViewRef) {
  previewOpener = opener;
  selected.set(photo);
}
function closePreview() {
  selected.set(null);
  const opener = previewOpener;
  previewOpener = undefined;
  if (opener) setTimeout(() => opener.focus(), 0);
}
export function columns(sidebarWidth: number) {
  return Math.max(1, Math.floor((windowSize().width - sidebarWidth - 40) / 230));
}
const section = () => (page() === "month" ? month() : page());
function CardActions(props: { photo: Photo; featured?: boolean }) {
  let previewButton: ViewRef | undefined;
  const busy = () => active().includes(props.photo.date);
  const progress = () => transfers().find((tr) => tr.date === props.photo.date && tr.state === "active");
  return (
    <View class="gap-1">
      <Toolbar class="gap-1" accessibilityLabel={props.photo.headline || props.photo.date}>
        <If when={props.featured}>
          <Action
            icon="eye"
            label={t("Preview")}
            buttonRef={(node) => {
              previewButton = node;
            }}
            press={() => openPreview(props.photo, previewButton)}
          />
        </If>
        <Action
          icon="monitor"
          id={
            (props.featured && props.photo.date !== photos()[0]?.date ? "hero-" : "card-") +
            "apply-" +
            props.photo.date
          }
          label={t("Set as wallpaper")}
          disabled={busy() || !canApply()}
          press={() => download(props.photo, true)}
        />
        <Action
          icon="download"
          id={
            (props.featured && props.photo.date !== photos()[0]?.date ? "hero-" : "card-") +
            "download-" +
            props.photo.date
          }
          label={t("Download")}
          disabled={busy()}
          press={() => download(props.photo, false)}
        />
        <Action
          icon="heart"
          id={
            (props.featured && props.photo.date !== photos()[0]?.date ? "hero-" : "card-") +
            "favorite-" +
            props.photo.date
          }
          filled={saved().includes(props.photo.date)}
          label={t(saved().includes(props.photo.date) ? "Cancel favorite" : "Favorite")}
          press={() => favorite(props.photo)}
        />
      </Toolbar>
      <If when={progress()}>
        {(tr) => (
          <Progress
            value={(100 * tr().received) / (tr().total || 1)}
            indeterminate={!tr().total}
            accessibilityLabel={t("Download progress")}
            trackClass="h-2 rounded bg-muted"
            indicatorClass="h-2 rounded bg-primary"
          />
        )}
      </If>
    </View>
  );
}
function Featured(props: { available: number; visible: boolean }) {
  const win = currentWindow();
  const slides = computed(() => photos().slice(0, 7));
  const index = signal(0),
    hovered = signal(false);
  const at = (delta = 0) => slides()[(index() + delta + slides().length) % slides().length];
  const step = (delta: number) => index.set((index() + delta + slides().length) % slides().length);
  const width = () => Math.max(1, Math.min(624, props.available * 0.68));
  let opener: ViewRef | undefined;
  effect(() => {
    if (win.state()?.visible === false || !props.visible || hovered() || selected() || slides().length < 2)
      return;
    const timer = setInterval(() => step(1), 4000);
    return () => clearInterval(timer);
  });
  function side(delta: number) {
    return (
      <Button
        testID={delta < 0 ? "hero-prev" : "hero-next"}
        accessibilityLabel={t(delta < 0 ? "Previous" : "Next")}
        onPress={() => step(delta)}
        class="flex-1 h-full overflow-hidden rounded-xl justify-center bg-muted"
      >
        <Image
          src={at(delta).thumbnail}
          style={{ width: "100%", height: "88%", objectFit: "cover", filter: "blur(5px)", opacity: 0.75 }}
        />
      </Button>
    );
  }
  return (
    <View class="gap-1" onMouseEnter={() => hovered.set(true)} onMouseLeave={() => hovered.set(false)}>
      <View
        class="flex-row items-center justify-center gap-2 overflow-hidden"
        style={{ height: (width() * 9) / 16 }}
      >
        <If when={slides().length > 1}>{side(-1)}</If>
        <Button
          testID="latest-photo"
          ref={(node) => {
            opener = node;
          }}
          accessibilityLabel={t("Preview") + " · " + at().date}
          onPress={() => openPreview(at(), opener)}
          class="overflow-hidden rounded-xl bg-muted focus-visible:ring-2 focus-visible:ring-ring"
          style={{ width: width(), height: "100%" }}
        >
          <Image src={at().preview} style={{ width: "100%", height: "100%", objectFit: "cover" }} />
          <View class="absolute bottom-0 inset-x-0 items-center px-3 pt-4 pb-2" style={captionShade}>
            <Text class="text-sm font-semibold text-white text-center line-clamp-1">
              {at().headline || at().date}
            </Text>
          </View>
        </Button>
        <If when={slides().length > 1}>{side(1)}</If>
      </View>
      <View class="flex-row items-center justify-center">
        <CardActions featured photo={at()} />
      </View>
    </View>
  );
}
const captionShade = { backgroundImage: "linear-gradient(180deg, rgba(0,0,0,0), rgba(0,0,0,0.65))" };
export function Gallery(props: { sidebarWidth: number }) {
  const scrolled = signal(false);
  let grid: ListRef | undefined;
  let previous = "";
  let version = 0;
  const list = computed(() => (page() === "home" ? photos().slice(1) : entries()));
  effect(() => {
    const key = section(),
      count = columns(props.sidebarWidth);
    if (!count || !grid || previous === key + count) return;
    previous = key + count;
    scrolled.set((positions.get(key) ?? 0) > 0);
    const index = positions.get(key) ?? 0;
    const turn = ++version;
    setTimeout(() => {
      if (turn !== version || section() !== key || !grid) return;
      if (index) grid.scrollToIndex({ index, animated: false });
      else grid.scrollToOffset({ offset: 0, animated: false });
    }, 0);
  });
  return (
    <View class="flex-1 gap-2">
      <If when={page() === "favorites" && !entries().length}>
        <Text class="text-xs text-muted-foreground">{t("No favorite wallpapers yet")}</Text>
      </If>
      <List
        smoothWheel={false}
        testID="wallpaper-grid"
        ref={(node) => {
          grid = node;
        }}
        each={list()}
        keyExtractor={(p) => p.date}
        class="flex-1"
        header={
          <If when={page() === "home" && photos().length > 0}>
            <Featured available={windowSize().width - props.sidebarWidth - 30} visible={!scrolled()} />
          </If>
        }
        numColumns={columns(props.sidebarWidth)}
        estimatedItemSize={
          Math.round(
            (((windowSize().width - props.sidebarWidth - 24) / columns(props.sidebarWidth)) * 9) / 16,
          ) + 32
        }
        initialScrollIndex={positions.get(section()) || undefined}
        onScrollEnd={({ offset }) => scrolled.set(offset.y > 12)}
        onViewableItemsChange={({ viewableItems }) => {
          if (viewableItems.length) {
            positions.set(section(), viewableItems[0].index);
            if (viewableItems[0].index > 0) scrolled.set(true);
          }
        }}
        contentStyle={{ gap: 6 }}
      >
        {(photo) => {
          let opener: ViewRef | undefined;
          const hovered = signal(false),
            missing = signal(false);
          return (
            <View
              class="overflow-hidden rounded-lg bg-card"
              onMouseEnter={() => hovered.set(true)}
              onMouseLeave={() => hovered.set(false)}
            >
              <Button
                ref={(node) => {
                  opener = node;
                }}
                testID={"photo-" + photo().date}
                onPress={() => openPreview(photo(), opener)}
                accessibilityLabel={t("Preview") + " · " + photo().date + " · " + (photo().headline ?? "")}
                class="hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring active:opacity-80"
              >
                <Thumbnail src={photo().thumbnail} failed={() => missing.set(true)} />
                <If when={hovered() || missing() || !photo().thumbnail}>
                  <View
                    class="absolute bottom-0 inset-x-0 items-center p-1"
                    style={{ backgroundColor: "rgba(255,255,255,0.85)", backdropFilter: "blur(8px)" }}
                  >
                    <Text class="text-xs font-semibold text-black text-center line-clamp-1">
                      {photo().headline || photo().date}
                    </Text>
                  </View>
                </If>
              </Button>
              <View class="flex-row items-center justify-between gap-1 px-2 pb-1">
                <Text class="text-xs text-muted-foreground">{photo().date}</Text>
                <CardActions photo={photo()} />
              </View>
            </View>
          );
        }}
      </List>
      <If when={scrolled()}>
        <View class="absolute bottom-3 right-4 rounded-full bg-card border border-border shadow-sm">
          <Action
            id="gallery-top"
            icon="arrow-up"
            label={t("Back to top")}
            press={() => {
              grid?.scrollToOffset({ offset: 0, animated: false });
              scrolled.set(false);
            }}
          />
        </View>
      </If>
    </View>
  );
}
export function Preview(props: { photo: Photo }) {
  effect(() => () => {
    setTimeout(releaseUnusedImages, 0);
  });
  const item = () => props.photo;
  const list = () => entries();
  const index = () => list().findIndex((p) => p.date === item().date);
  function step(delta: number) {
    const next = list()[index() + delta];
    if (next) selected.set(next);
  }
  keymap.bind({ left: () => step(-1), right: () => step(1), escape: closePreview });
  const transfer = () => transfers().find((tr) => tr.date === item().date && tr.state === "active");
  return (
    <PhotoViewer
      identity={item().date}
      title={item().headline || item().date}
      closeId="close-preview"
      close={closePreview}
      photo={() => (
        <Image src={item().preview} style={{ width: "100%", height: "100%", objectFit: "contain" }} />
      )}
      details={() => (
        <View>
          <Text class="text-xs text-white text-center line-clamp-2">
            {item().date} · {item().description}
          </Text>
          <If when={transfer()}>
            {(tr) => (
              <Progress
                value={(100 * tr().received) / (tr().total || 1)}
                indeterminate={!tr().total}
                accessibilityLabel={t("Download progress")}
                trackClass="h-2 rounded bg-muted"
                indicatorClass="h-2 rounded bg-primary"
              />
            )}
          </If>
        </View>
      )}
      actions={() => (
        <View class="flex-row items-center">
          <Action
            icon="chevron-left"
            id="previous-photo"
            label={t("Previous")}
            disabled={index() <= 0}
            press={() => step(-1)}
          />
          <Action
            icon="chevron-right"
            id="next-photo"
            label={t("Next")}
            disabled={index() + 1 >= list().length}
            press={() => step(1)}
          />
          <Action
            icon="heart"
            id="toggle-favorite"
            filled={saved().includes(item().date)}
            label={t(saved().includes(item().date) ? "Cancel favorite" : "Favorite")}
            press={() => favorite(item())}
          />
          <Action
            icon="download"
            id="download-photo"
            label={t("Download")}
            disabled={active().includes(item().date)}
            press={() => download(item(), false)}
          />
          <Action
            icon="monitor"
            id="apply-photo"
            tint
            label={t("Set as wallpaper")}
            disabled={!canApply() || active().includes(item().date)}
            press={() => download(item(), true)}
          />
        </View>
      )}
    />
  );
}
