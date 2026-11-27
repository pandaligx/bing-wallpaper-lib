import { effect, Image, List, mount, signal, Text, View, windowSize } from "nsh";
import { Button, Dialog, Progress } from "nsh/base";
import { Window } from "nsh/desktop";
import { Action } from "./src/ui/Action";
import { WallpaperOptions } from "./src/ui/WallpaperOptions";
import { active, download, favorite, load, photos, refresh, saved, selected, setSource, source, status } from "./src/library";
import type { Photo } from "./src/library";
import { checkUpdate, pauseTransfer, pollTransfers, transfers } from "./src/library";

function Preview(props: { photo: Photo }) {
  const item = () => props.photo;
  const transfer = () => transfers().find(t => t.date === item().date && (t.state === "active" || t.state === "waiting"));
  function step(delta: number) {
    const all = photos();
    const next = all[all.findIndex(p => p.date === item().date) + delta];
    if (next) selected.set(next);
  }
  return (
    <Dialog open animation="fade" class="m-auto rounded-xl bg-card" accessibilityLabel={item().headline ?? item().date}
      onOpenChange={({open}) => { if (!open) selected.set(null); }}>
        <View class="gap-3 rounded-xl bg-card p-5" style={{width: Math.max(248, Math.min(880, windowSize().width - 40, (windowSize().height - 220) * 16 / 9))}}>
        <Image src={item().preview} style={{width:"100%", aspectRatio:16/9, borderRadius:8}} />
        <Text class="text-xl font-semibold text-foreground">{item().headline || item().date}</Text>
        <Text class="text-muted-foreground">{item().date} · {item().description}</Text>
        <If when={active().includes(item().date) ? transfer() : undefined}>{(value) =>
          <View class="gap-1">
            <Progress accessibilityLabel="下载进度" indeterminate={!value().total}
              value={100 * value().received / (value().total || 1)}
              trackClass="h-2 rounded bg-muted" indicatorClass="h-2 rounded bg-primary" />
            <Text class="text-muted-foreground">{(value().received / 1048576).toFixed(1)} MiB · {Math.round(value().speed / 1024)} KiB/s</Text>
            <Action label="暂停下载" press={() => void pauseTransfer(value().id)} />
          </View>
        }</If>
        <View class="flex-row flex-wrap gap-2">
          <Action label="上一张" press={() => step(-1)} />
          <Action label="下一张" press={() => step(1)} />
          <Action label={saved().includes(item().date) ? "取消收藏" : "收藏"} press={() => favorite(item())} />
          <Action label="下载" disabled={active().includes(item().date)} press={() => void download(item(), false)} />
          <Action primary label="设为桌面壁纸" disabled={active().includes(item().date)} press={() => void download(item(), true)} />
          <Action label="关闭" press={() => selected.set(null)} />
        </View>
      </View>
    </Dialog>
  );
}

function App() {
  const favoritesOnly = signal(false);
  const regionOpen = signal(false);
  const ready = signal(false);
  effect(() => {
    if (!active().length) return;
    pollTransfers();
    const timer = setInterval(pollTransfers, 250);
    return () => clearInterval(timer);
  });
  void load().then(() => { ready.set(true); if (source()) void refresh(); }).catch(e => status.set(String(e)));
  effect(() => {
    if (!ready() || !source()) return;
    const timer = setInterval(() => void refresh(), 30 * 60 * 1000);
    return () => clearInterval(timer);
  });
  const entries = () => favoritesOnly() ? photos().filter(p => saved().includes(p.date)) : photos();
  return (
    <View class="flex-1 gap-4 bg-background p-5">
      <View class="flex-row flex-wrap items-center justify-between gap-2">
        <Text class="text-2xl font-semibold text-foreground">{favoritesOnly() ? "我的收藏" : "必应每日壁纸库"}</Text>
        <View class="flex-row gap-2">
          <Action label="全部壁纸" press={() => favoritesOnly.set(false)} />
          <Action label="我的收藏" press={() => favoritesOnly.set(true)} />
          <Action label={source() ? `设置 · ${source()}` : "设置"} press={() => regionOpen.set(!regionOpen())} />
        </View>
      </View>
      <If when={ready() && (regionOpen() || !source())}>
        <View class="flex-row flex-wrap items-center gap-2">
          <Text class="text-foreground">选择更新地区</Text>
          <Action label="跟随系统地区" press={() => { setSource("auto"); regionOpen.set(false); }} />
          <Action label="中国大陆 · GitCode" press={() => { setSource("cn"); regionOpen.set(false); }} />
          <Action label="国际 · GitHub" press={() => { setSource("international"); regionOpen.set(false); }} />
          <Action label="检查更新" disabled={!source()} press={() => void checkUpdate()} />
          <Action label="下载已验证更新包" disabled={!source()} press={() => void checkUpdate(true)} />
        </View>
      </If>
      <If when={regionOpen()}><WallpaperOptions /></If>
      <Text class="text-muted-foreground">{ready() ? `${entries().length} 张壁纸` : "正在读取本地壁纸库"}</Text>
      <List each={entries()} keyExtractor={p => p.date} class="flex-1"
        numColumns={Math.max(1, Math.floor((windowSize().width - 40) / 280))}
        estimatedItemSize={236} contentStyle={{gap:12}}>
        {(photo) => (
          <Button testID={photo().date === photos()[0]?.date ? "latest-photo" : "photo-" + photo().date} onPress={() => selected.set(photo())} class="overflow-hidden rounded-xl border border-border bg-card hover:bg-accent active:opacity-80">
            <Image src={photo().thumbnail} style={{width:"100%", aspectRatio:16/9}} />
            <View class="gap-1 px-3 py-2">
              <Text class="font-semibold text-foreground">{photo().headline || photo().date}</Text>
              <Text class="text-sm text-muted-foreground">{photo().date}</Text>
            </View>
          </Button>
        )}
      </List>
      <If when={!!status()}><Text class="text-muted-foreground">{status()}</Text></If>
      <If when={selected()}>{(photo) => <Preview photo={photo()} />}</If>
    </View>
  );
}

mount(() => <Window id="main" title="必应每日壁纸库">{App}</Window>);
