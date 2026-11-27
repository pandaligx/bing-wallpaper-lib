import { computed, effect, Image, keymap, List, Text, View, windowSize } from "nsh";
import type { ListRef, ViewRef } from "nsh";
import { Button, Dialog, Progress } from "nsh/base";
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
export function columns() {
  return Math.max(1, Math.floor((windowSize().width - (windowSize().width >= 800 ? 240 : 40)) / 230));
}
const section = () => (page() === "month" ? month() : page());
function Featured(props: { photo: Photo }) {
  let imageButton: ViewRef | undefined;
  let previewButton: ViewRef | undefined;
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
      <View class="flex-1 justify-center gap-2 p-4">
        <Text class="text-sm text-muted-foreground">{photo().date}</Text>
        <Text class="text-xl font-semibold text-foreground">{photo().headline || photo().date}</Text>
        <Text class="text-sm text-muted-foreground">{photo().description}</Text>
        <View class="flex-row flex-wrap gap-2">
          <Action
            primary
            label={t("Preview")}
            buttonRef={(node) => {
              previewButton = node;
            }}
            press={() => openPreview(photo(), previewButton)}
          />
          <Action
            label={t(saved().includes(photo().date) ? "Cancel favorite" : "Favorite")}
            press={() => favorite(photo())}
          />
        </View>
      </View>
    </View>
  );
}
export function Gallery() {
  let grid: ListRef | undefined;
  let previous = "";
  let version = 0;
  const list = computed(() => (page() === "home" ? photos().slice(1) : entries()));
  effect(() => {
    const key = section(),
      count = columns();
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
      <View class="flex-row items-center justify-between gap-2">
        <Text class="text-muted-foreground">
          {entries().length} {t("Wallpapers")}
        </Text>
        <Action label={t("Back to top")} press={() => grid?.scrollToOffset({ offset: 0, animated: true })} />
      </View>
      <If when={page() === "favorites" && !entries().length}>
        <Text class="text-muted-foreground">{t("No favorite wallpapers yet")}</Text>
      </If>
      <List
        testID="wallpaper-grid"
        ref={(node) => {
          grid = node;
        }}
        each={list()}
        keyExtractor={(p) => p.date}
        class="flex-1"
        header={<If when={page() === "home" ? photos()[0] : undefined}>{(p) => <Featured photo={p()} />}</If>}
        numColumns={columns()}
        estimatedItemSize={206}
        initialScrollIndex={positions.get(section()) || undefined}
        onViewableItemsChange={({ viewableItems }) => {
          if (viewableItems.length) positions.set(section(), viewableItems[0].index);
        }}
        contentStyle={{ gap: 10 }}
      >
        {(photo) => {
          let opener: ViewRef | undefined;
          return (
            <Button
              ref={(node) => {
                opener = node;
              }}
              testID={"photo-" + photo().date}
              onPress={() => openPreview(photo(), opener)}
              class="overflow-hidden rounded-lg border border-border bg-card hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring active:opacity-80"
            >
              <Thumbnail src={photo().thumbnail} />
              <View class="gap-1 px-3 py-2">
                <Text class="font-semibold text-foreground">{photo().headline || photo().date}</Text>
                <Text class="text-sm text-muted-foreground">
                  {photo().date}
                  {saved().includes(photo().date) ? "  ★" : ""}
                </Text>
              </View>
            </Button>
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
      class="m-auto rounded-xl bg-card"
      accessibilityLabel={item().headline ?? item().date}
      onOpen={() => closeButton?.focus()}
      onOpenChange={({ open }) => {
        if (!open) closePreview();
      }}
    >
      <View
        class="gap-3 rounded-xl bg-card p-4"
        style={{
          width: Math.max(
            248,
            Math.min(900, windowSize().width - 32, ((windowSize().height - 230) * 16) / 9),
          ),
        }}
      >
        <Image src={item().preview} style={{ width: "100%", aspectRatio: 16 / 9, borderRadius: 6 }} />
        <Text class="text-xl font-semibold text-foreground">{item().headline || item().date}</Text>
        <Text class="text-muted-foreground">
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
        <View class="flex-row flex-wrap gap-2">
          <Action id="previous-photo" label={t("Previous")} disabled={index() <= 0} press={() => step(-1)} />
          <Action
            id="next-photo"
            label={t("Next")}
            disabled={index() + 1 >= list().length}
            press={() => step(1)}
          />
          <Action
            id="toggle-favorite"
            label={t(saved().includes(item().date) ? "Cancel favorite" : "Favorite")}
            press={() => favorite(item())}
          />
          <Action
            id="download-photo"
            label={t("Download")}
            disabled={active().includes(item().date)}
            press={() => download(item(), false)}
          />
          <Action
            id="apply-photo"
            primary
            label={t("Set as wallpaper")}
            disabled={!canApply() || active().includes(item().date)}
            press={() => download(item(), true)}
          />
          <Action
            id="close-preview"
            label={t("Close")}
            buttonRef={(node) => {
              closeButton = node;
            }}
            press={closePreview}
          />
        </View>
      </View>
    </Dialog>
  );
}
