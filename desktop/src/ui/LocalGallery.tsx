import { effect, Image, keymap, List, signal, View, windowSize } from "nsh";
import { Text } from "./Text";
import type { ViewRef } from "nsh";
import { Button, Dialog } from "nsh/base";
import { Library } from "wallpaper-native";
import { canApply, run, notify, reportError, t } from "../preferences";
import { PhotoViewer } from "./PhotoViewer";
import { localFiles as files, scanLocalFiles } from "../library";
import { columns } from "./Gallery";
import { Action } from "./Action";
import { releaseUnusedImages } from "./Thumbnail";

function LocalImage(props: {
  name: string;
  preview?: boolean;
  revision?: number;
  cache: Map<string, string>;
}) {
  const src = signal("");
  effect(() => {
    void props.revision;
    const name = props.name,
      preview = !!props.preview;
    let visible = true;
    const cached = preview ? undefined : props.cache.get(name);
    src.set(cached ?? "");
    if (cached) {
      props.cache.delete(name);
      props.cache.set(name, cached);
    } else
      void Library.localImage(name, preview)
        .then((data) => {
          if (!visible) return;
          if (!preview) {
            props.cache.set(name, data);
            if (props.cache.size > 120) props.cache.delete(props.cache.keys().next().value!);
          }
          src.set(data);
        })
        .catch((error) => {
          if (visible) reportError(error);
        });
    return () => {
      visible = false;
      if (preview) setTimeout(releaseUnusedImages, 0);
    };
  });
  return (
    <View
      testID={props.preview ? "local-preview-image" : "local-thumbnail-" + props.name}
      class={props.preview ? "" : "bg-muted overflow-hidden shrink-0"}
      style={{ width: "100%", aspectRatio: 16 / 9 }}
    >
      <If when={src()}>
        {(url) => (
          <Image
            src={url()}
            style={{ width: "100%", height: "100%", objectFit: props.preview ? "contain" : "cover" }}
          />
        )}
      </If>
    </View>
  );
}
function FileDetails(props: { name: string; revision: number; overlay?: boolean }) {
  const details = signal("");
  effect(() => {
    void props.revision;
    let visible = true;
    details.set("");
    void Library.localDetails(props.name)
      .then((raw) => {
        if (!visible) return;
        const info = JSON.parse(raw);
        details.set(info.width + " × " + info.height + " · " + (info.bytes / 1048576).toFixed(2) + " MiB");
      })
      .catch((error) => {
        if (visible) details.set(String(error));
      });
    return () => {
      visible = false;
    };
  });
  return (
    <Text
      class={
        props.overlay
          ? "text-xs text-white text-center line-clamp-1"
          : "text-xs text-muted-foreground text-center line-clamp-1"
      }
    >
      {details()}
    </Text>
  );
}
export function LocalGallery(props: { sidebarWidth: number }) {
  let previewOpener: ViewRef | undefined;
  const closePreview = () => {
    preview.set("");
    const opener = previewOpener;
    previewOpener = undefined;
    if (opener) setTimeout(() => opener.focus(), 0);
  };
  const selection = signal<string[]>([]),
    preview = signal(""),
    confirm = signal<string[]>([]);
  const revision = signal(0);
  effect(() => {
    if (!preview() || confirm().length) return;
    return keymap.bind({ left: () => step(-1), right: () => step(1), escape: closePreview });
  });
  const cache = new Map<string, string>();
  async function scan() {
    cache.clear();
    await scanLocalFiles();
    const names = files();
    revision.update((v) => v + 1);
    selection.update((s) => s.filter((name) => names.includes(name)));
  }
  run(scan);
  function toggle(name: string) {
    selection.update((s) => (s.includes(name) ? s.filter((n) => n !== name) : [...s, name]));
  }
  function applyLocal(name: string) {
    run(async () => {
      await Library.applyLocal(name);
      notify(t("Applied"));
    });
  }
  async function remove() {
    const names = confirm();
    confirm.set([]);
    const result: { deleted: string[]; failed: { name: string; error: string }[] } = JSON.parse(
      await Library.deleteLocal(JSON.stringify(names)),
    );
    if (result.deleted.includes(preview())) preview.set("");
    const feedback = result.failed.length ? reportError : notify;
    feedback(
      t("Delete") +
        " · " +
        result.deleted.length +
        (result.failed.length ? " · " + result.failed.map((f) => f.name + " " + f.error).join(", ") : ""),
    );
    await scan();
  }
  const step = (delta: number) => {
    const name = files()[files().indexOf(preview()) + delta];
    if (name) preview.set(name);
  };
  return (
    <View class="flex-1 gap-2">
      <View class="flex-row flex-wrap gap-2">
        <Action icon="refresh-cw" id="scan-local" label={t("Refresh")} press={() => run(scan)} />
        <Action
          icon="folder-open"
          label={t("Open folder")}
          press={() => run(() => Library.openLocation("folder"))}
        />
        <Action
          icon="list-checks"
          label={t("Select all")}
          disabled={!files().length}
          press={() => selection.set([...files()])}
        />
        <Action
          icon="list-x"
          label={t("Clear selection")}
          disabled={!selection().length}
          press={() => selection.set([])}
        />
        <Action
          icon="trash-2"
          id="delete-selected"
          label={t("Delete selected") + " · " + selection().length}
          disabled={!selection().length}
          press={() => confirm.set([...selection()])}
        />
      </View>
      <If when={!files().length}>
        <Text class="text-xs text-muted-foreground">{t("No downloaded wallpapers yet")}</Text>
      </If>
      <List
        smoothWheel={false}
        each={files()}
        keyExtractor={(name) => name}
        numColumns={columns(props.sidebarWidth)}
        estimatedItemSize={210}
        class="flex-1"
        contentStyle={{ gap: 6 }}
      >
        {(name) => {
          let button: ViewRef | undefined;
          return (
            <View class="w-full min-w-0 gap-1 overflow-hidden rounded-lg border border-border bg-card">
              <Button
                ref={(node) => {
                  button = node;
                }}
                onPress={() => {
                  previewOpener = button;
                  preview.set(name());
                }}
                accessibilityLabel={t("Preview") + " " + name()}
                class="w-full min-w-0 focus-visible:ring-2 focus-visible:ring-ring"
              >
                <LocalImage name={name()} revision={revision()} cache={cache} />
                <Text class="text-xs px-2 py-1 text-foreground line-clamp-1 text-center">{name()}</Text>
                <FileDetails name={name()} revision={revision()} />
              </Button>
              <View class="flex-row flex-wrap gap-1 px-2 pb-2">
                <Action
                  icon="monitor"
                  id={"apply-local-" + name()}
                  label={t("Set as wallpaper")}
                  disabled={!canApply()}
                  press={() => applyLocal(name())}
                />
                <Action
                  icon="square-check"
                  id={"select-" + name()}
                  primary={selection().includes(name())}
                  label={t("Select")}
                  press={() => toggle(name())}
                />
                <Action
                  icon="trash-2"
                  id={"delete-local-" + name()}
                  label={t("Delete")}
                  press={() => confirm.set([name()])}
                />
              </View>
            </View>
          );
        }}
      </List>
      <If when={preview()}>
        {(name) => (
          <PhotoViewer
            identity={name()}
            title={name()}
            closeId="close-local"
            close={closePreview}
            photo={() => <LocalImage name={name()} preview cache={cache} />}
            details={() => <FileDetails name={name()} revision={revision()} overlay />}
            actions={() => (
              <View class="flex-row items-center">
                <Action icon="chevron-left" label={t("Previous")} press={() => step(-1)} />
                <Action icon="chevron-right" label={t("Next")} press={() => step(1)} />
                <Action
                  icon="monitor"
                  tint
                  disabled={!canApply()}
                  label={t("Set as wallpaper")}
                  press={() => applyLocal(name())}
                />
                <Action icon="trash-2" label={t("Delete")} press={() => confirm.set([name()])} />
              </View>
            )}
          />
        )}
      </If>
      <If when={confirm().length > 0}>
        <Dialog
          open
          class="m-auto rounded-lg bg-card"
          onOpenChange={({ open }) => {
            if (!open) confirm.set([]);
          }}
        >
          <View class="gap-3 p-5">
            <Text class="text-sm font-semibold text-foreground">
              {t("Delete selected")} ({confirm().length})?
            </Text>
            <View class="flex-row gap-2">
              <Action id="cancel-delete" label={t("Cancel")} press={() => confirm.set([])} />
              <Action id="confirm-delete" primary label={t("Delete")} press={() => run(remove)} />
            </View>
          </View>
        </Dialog>
      </If>
    </View>
  );
}
