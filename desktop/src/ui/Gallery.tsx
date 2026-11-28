import { smoothWheel } from "../preferences";
import { computed, effect, Image, keymap, List, Text, View, windowSize } from "nsh";
import type { ListRef, ViewRef } from "nsh";
import { Button, Dialog, Progress, Toolbar } from "nsh/base";
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
          id={"card-apply-" + props.photo.date}
          label={t("Set as wallpaper")}
          disabled={busy() || !canApply()}
          press={() => download(props.photo, true)}
        />
        <Action
          icon="download"
          id={"card-download-" + props.photo.date}
          label={t("Download")}
          disabled={busy()}
          press={() => download(props.photo, false)}
        />
        <Action
          icon="heart"
          id={"card-favorite-" + props.photo.date}
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
function Featured(props: { photo: Photo }) {
  let imageButton: ViewRef | undefined;
  const photo = () => props.photo;
  const wide = () => windowSize().width >= 700;
  return (
    <View
      class={
        wide()
          ? "flex-row overflow-hidden rounded-lg border border-border bg-card"
          : "overflow-hidden rounded-lg border border-border bg-card"
      }
    >
      <Button
        testID="latest-photo"
        ref={(node) => {
          imageButton = node;
        }}
        accessibilityLabel={t("Preview") + " · " + photo().date}
        onPress={() => openPreview(photo(), imageButton)}
        style={{ width: wide() ? "54%" : "100%" }}
      >
        <Image src={photo().preview} style={{ width: "100%", aspectRatio: 16 / 9 }} />
      </Button>
      <View class="flex-1 justify-center gap-1 p-3">
        <Text class="text-xs text-muted-foreground">{photo().date}</Text>
        <Text class="text-sm font-semibold text-foreground">{photo().headline || photo().date}</Text>
        <Text class="text-xs text-muted-foreground">{photo().description}</Text>
        <CardActions featured photo={photo()} />
      </View>
    </View>
  );
}
export function Gallery(props: { sidebarWidth: number; listRef: (node: ListRef) => void }) {
  let grid: ListRef | undefined;
  let previous = "";
  let version = 0;
  const list = computed(() => (page() === "home" ? photos().slice(1) : entries()));
  effect(() => {
    const key = section(),
      count = columns(props.sidebarWidth);
    if (!count || !grid || previous === key + count) return;
    previous = key + count;
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
        smoothWheel={smoothWheel()}
        testID="wallpaper-grid"
        ref={(node) => {
          grid = node;
          props.listRef(node);
        }}
        each={list()}
        keyExtractor={(p) => p.date}
        class="flex-1"
        header={<If when={page() === "home" ? photos()[0] : undefined}>{(p) => <Featured photo={p()} />}</If>}
        numColumns={columns(props.sidebarWidth)}
        estimatedItemSize={
          Math.round(
            (((windowSize().width - props.sidebarWidth - 24) / columns(props.sidebarWidth)) * 9) / 16,
          ) + 56
        }
        initialScrollIndex={positions.get(section()) || undefined}
        onViewableItemsChange={({ viewableItems }) => {
          if (viewableItems.length) positions.set(section(), viewableItems[0].index);
        }}
        contentStyle={{ gap: 6 }}
      >
        {(photo) => {
          let opener: ViewRef | undefined;
          return (
            <View class="overflow-hidden rounded-lg border border-border bg-card">
              <Button
                ref={(node) => {
                  opener = node;
                }}
                testID={"photo-" + photo().date}
                onPress={() => openPreview(photo(), opener)}
                accessibilityLabel={t("Preview") + " · " + photo().date}
                class="hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring active:opacity-80"
              >
                <Thumbnail src={photo().thumbnail} />
                <View class="gap-1 px-2 pt-1">
                  <Text class="text-xs font-semibold text-foreground line-clamp-2">
                    {photo().headline || photo().date}
                  </Text>
                </View>
              </Button>
              <View class="flex-row items-center justify-between gap-1 px-2 pb-1">
                <Text class="text-xs text-muted-foreground">{photo().date}</Text>
                <CardActions photo={photo()} />
              </View>
            </View>
          );
        }}
      </List>
    </View>
  );
}
export function Preview(props: { photo: Photo }) {
  let closeButton: ViewRef | undefined;
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
    <Dialog
      open
      animation="fade"
      class="m-auto rounded-lg bg-card"
      accessibilityLabel={item().headline ?? item().date}
      onOpen={() => closeButton?.focus()}
      onOpenChange={({ open }) => {
        if (!open) closePreview();
      }}
    >
      <View
        class="gap-3 rounded-lg bg-card p-4"
        style={{
          width: Math.max(
            248,
            Math.min(900, windowSize().width - 32, ((windowSize().height - 230) * 16) / 9),
          ),
        }}
      >
        <Image src={item().preview} style={{ width: "100%", aspectRatio: 16 / 9, borderRadius: 6 }} />
        <Text class="text-sm font-semibold text-foreground">{item().headline || item().date}</Text>
        <Text class="text-xs text-muted-foreground">
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
        <Toolbar class="gap-1" accessibilityLabel={t("Preview")}>
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
            primary
            label={t("Set as wallpaper")}
            disabled={!canApply() || active().includes(item().date)}
            press={() => download(item(), true)}
          />
          <Action
            icon="x"
            id="close-preview"
            label={t("Close")}
            buttonRef={(node) => {
              closeButton = node;
            }}
            press={closePreview}
          />
        </Toolbar>
      </View>
    </Dialog>
  );
}
