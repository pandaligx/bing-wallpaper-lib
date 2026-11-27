import { effect, Image, List, signal, Text, View, windowSize } from "nsh";
import { Button, Dialog } from "nsh/base";
import { Library } from "wallpaper-native";
import { canApply, run, status, t } from "../preferences";
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
          if (visible) status.set(String(error));
        });
    return () => {
      visible = false;
      if (preview) setTimeout(releaseUnusedImages, 0);
    };
  });
  return (
    <View class="bg-muted" style={{ width: "100%", aspectRatio: 16 / 9 }}>
      <If when={src()}>{(url) => <Image src={url()} style={{ width: "100%", height: "100%" }} />}</If>
    </View>
  );
}
export function LocalGallery() {
  const files = signal<string[]>([]),
    selection = signal<string[]>([]),
    preview = signal(""),
    confirm = signal<string[]>([]);
  const revision = signal(0),
    details = signal("");
  effect(() => {
    const name = preview();
    let visible = true;
    details.set("");
    if (name)
      void Library.localDetails(name)
        .then((raw) => {
          if (visible) {
            const info = JSON.parse(raw);
            details.set(
              info.width + " × " + info.height + " · " + (info.bytes / 1048576).toFixed(2) + " MiB",
            );
          }
        })
        .catch((error) => {
          if (visible) status.set(String(error));
        });
    return () => {
      visible = false;
    };
  });
  const cache = new Map<string, string>();
  async function scan() {
    cache.clear();
    const names: string[] = JSON.parse(await Library.localFiles());
    files.set(names);
    revision.update((v) => v + 1);
    selection.update((s) => s.filter((name) => names.includes(name)));
  }
  run(scan);
  function toggle(name: string) {
    selection.update((s) => (s.includes(name) ? s.filter((n) => n !== name) : [...s, name]));
  }
  async function remove() {
    const names = confirm();
    confirm.set([]);
    const result: { deleted: string[]; failed: { name: string; error: string }[] } = JSON.parse(
      await Library.deleteLocal(JSON.stringify(names)),
    );
    if (result.deleted.includes(preview())) preview.set("");
    status.set(
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
    <View class="flex-1 gap-3">
      <View class="flex-row flex-wrap gap-2">
        <Action id="scan-local" label={t("Refresh")} press={() => run(scan)} />
        <Action label={t("Open folder")} press={() => run(() => Library.openLocation("folder"))} />
        <Action
          label={t("Select all")}
          disabled={!files().length}
          press={() => selection.set([...files()])}
        />
        <Action label={t("Clear selection")} disabled={!selection().length} press={() => selection.set([])} />
        <Action
          id="delete-selected"
          label={t("Delete selected") + " · " + selection().length}
          disabled={!selection().length}
          press={() => confirm.set([...selection()])}
        />
      </View>
      <If when={!files().length}>
        <Text class="text-muted-foreground">{t("No downloaded wallpapers yet")}</Text>
      </If>
      <List
        each={files()}
        keyExtractor={(name) => name}
        numColumns={columns()}
        estimatedItemSize={210}
        class="flex-1"
        contentStyle={{ gap: 10 }}
      >
        {(name) => (
          <View class="gap-1 overflow-hidden rounded-lg border border-border bg-card">
            <Button onPress={() => preview.set(name())} accessibilityLabel={t("Preview") + " " + name()}>
              <LocalImage name={name()} revision={revision()} cache={cache} />
              <Text class="px-2 py-1 text-foreground">{name()}</Text>
            </Button>
            <Action
              id={"select-" + name()}
              primary={selection().includes(name())}
              label={t("Select")}
              press={() => toggle(name())}
            />
          </View>
        )}
      </List>
      <If when={preview()}>
        {(name) => (
          <Dialog
            open
            class="m-auto rounded-xl bg-card"
            onOpenChange={({ open }) => {
              if (!open) preview.set("");
            }}
          >
            <View
              class="gap-3 p-4"
              style={{
                width: Math.max(
                  250,
                  Math.min(860, windowSize().width - 32, ((windowSize().height - 196) * 16) / 9),
                ),
              }}
              onKeyDown={(e) => {
                if (e.key === "ArrowRight") step(1);
                if (e.key === "ArrowLeft") step(-1);
                if (e.key === "Escape") preview.set("");
              }}
            >
              <LocalImage name={name()} preview cache={cache} />
              <Text class="font-semibold text-foreground">{name()}</Text>
              <Text class="text-muted-foreground">{details()}</Text>
              <View class="flex-row flex-wrap gap-2">
                <Action label={t("Previous")} press={() => step(-1)} />
                <Action label={t("Next")} press={() => step(1)} />
                <Action
                  primary
                  disabled={!canApply()}
                  label={t("Set as wallpaper")}
                  press={() =>
                    run(async () => {
                      await Library.applyLocal(name());
                      status.set(t("Applied"));
                    })
                  }
                />
                <Action label={t("Delete")} press={() => confirm.set([name()])} />
                <Action id="close-local" label={t("Close")} press={() => preview.set("")} />
              </View>
            </View>
          </Dialog>
        )}
      </If>
      <If when={confirm().length > 0}>
        <Dialog
          open
          class="m-auto rounded-xl bg-card"
          onOpenChange={({ open }) => {
            if (!open) confirm.set([]);
          }}
        >
          <View class="gap-3 p-5">
            <Text class="text-lg font-semibold text-foreground">
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
