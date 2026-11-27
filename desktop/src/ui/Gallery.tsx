import { effect, Image, List, Text, View, windowSize } from "nsh";
import type { ListRef } from "nsh";
import { Button, Dialog, Progress } from "nsh/base";
import { active, download, entries, favorite, month, page, photos, saved, selected, transfers } from "../library";
import type { Photo } from "../library";
import { t } from "../preferences";
import { Action } from "./Action";

const positions = new Map<string, number>();
export function columns() { return Math.max(1, Math.floor((windowSize().width - (windowSize().width >= 800 ? 240 : 40)) / 230)); }
const section = () => page() === "month" ? month() : page();
export function Gallery() {
  let grid: ListRef | undefined;
  let previous = "";
  let version = 0;
  effect(() => {
    const key = section(), count = columns();
    if (!count || !grid || previous === key + count) return;
    previous = key + count;
    const index = positions.get(key) ?? 0;
    const turn = ++version;
    setTimeout(() => { if (turn === version && section() === key && grid) grid.scrollToIndex({index,animated:false}); },0);
  });
  return <View class="flex-1 gap-2">
    <View class="flex-row items-center justify-between gap-2">
      <Text class="text-muted-foreground">{entries().length} {t("Wallpapers")}</Text>
      <Action label={t("Back to top")} press={() => grid?.scrollToIndex({index:0,animated:true})}/>
    </View>
    <If when={page() === "favorites" && !entries().length}><Text class="text-muted-foreground">{t("No favorite wallpapers yet")}</Text></If>
    <List testID="wallpaper-grid" ref={node => {grid = node;}} each={entries()} keyExtractor={p => p.date} class="flex-1"
      numColumns={columns()} estimatedItemSize={206} initialScrollIndex={positions.get(section()) ?? 0}
      onViewableItemsChange={({viewableItems}) => { if (viewableItems.length) positions.set(section(),viewableItems[0].index); }}
      contentStyle={{gap:10}}>
      {photo => <Button testID={photo().date === photos()[0]?.date ? "latest-photo" : "photo-" + photo().date}
        onPress={() => selected.set(photo())} class="overflow-hidden rounded-lg border border-border bg-card hover:bg-accent active:opacity-80">
        <Image src={photo().thumbnail} style={{width:"100%",aspectRatio:16/9}}/>
        <View class="gap-1 px-3 py-2">
          <Text class="font-semibold text-foreground">{photo().headline || photo().date}</Text>
          <Text class="text-sm text-muted-foreground">{photo().date}{saved().includes(photo().date) ? "  ★" : ""}</Text>
        </View>
      </Button>}
    </List>
  </View>;
}
export function Preview(props:{photo:Photo}) {
  const item = () => props.photo;
  const list = () => entries();
  const index = () => list().findIndex(p => p.date === item().date);
  function step(delta:number) { const next = list()[index()+delta]; if(next) selected.set(next); }
  const transfer = () => transfers().find(tr => tr.date === item().date && tr.state === "active");
  return <Dialog open animation="fade" class="m-auto rounded-xl bg-card" accessibilityLabel={item().headline ?? item().date}
    onOpenChange={({open}) => {if(!open) selected.set(null);}}>
    <View class="gap-3 rounded-xl bg-card p-4" onKeyDown={e => {
      if(e.key === "ArrowLeft") step(-1);
      if(e.key === "ArrowRight") step(1);
      if(e.key === "Escape") selected.set(null);
    }} style={{width:Math.max(248,Math.min(900,windowSize().width-32,(windowSize().height-230)*16/9))}}>
      <Image src={item().preview} style={{width:"100%",aspectRatio:16/9,borderRadius:6}}/>
      <Text class="text-xl font-semibold text-foreground">{item().headline || item().date}</Text>
      <Text class="text-muted-foreground">{item().date} · {item().description}</Text>
      <If when={transfer()}>{tr => <Progress value={100*tr().received/(tr().total || 1)} indeterminate={!tr().total}
        accessibilityLabel={t("Download progress")} trackClass="h-2 rounded bg-muted" indicatorClass="h-2 rounded bg-primary"/>}</If>
      <View class="flex-row flex-wrap gap-2">
        <Action id="previous-photo" label={t("Previous")} disabled={index() <= 0} press={() => step(-1)}/>
        <Action id="next-photo" label={t("Next")} disabled={index()+1 >= list().length} press={() => step(1)}/>
        <Action id="toggle-favorite" label={t(saved().includes(item().date) ? "Cancel favorite" : "Favorite")} press={() => favorite(item())}/>
        <Action id="download-photo" label={t("Download")} disabled={active().includes(item().date)} press={() => download(item(),false)}/>
        <Action id="apply-photo" primary label={t("Set as wallpaper")} disabled={active().includes(item().date)} press={() => download(item(),true)}/>
        <Action id="close-preview" label={t("Close")} press={() => selected.set(null)}/>
      </View>
    </View>
  </Dialog>;
}
