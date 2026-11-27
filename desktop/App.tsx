import { computed, effect, mount, Scroll, signal, Text, View, windowSize } from "nsh";
import { Dialog } from "nsh/base";
import { Application, currentWindow, Tray, Window } from "nsh/desktop";
import { Library } from "wallpaper-native";
import { active, load, month, page, photos, pollTransfers, refresh, selected, source } from "./src/library";
import { applied, loadPreferences, preferences, run, status, t } from "./src/preferences";
import { Action } from "./src/ui/Action";
import { Gallery, Preview } from "./src/ui/Gallery";
import { LocalGallery } from "./src/ui/LocalGallery";
import { DownloadCenter } from "./src/ui/DownloadCenter";
import { Region, Settings } from "./src/ui/Settings";

const destinations = [{key:"home",label:"Home"},{key:"favorites",label:"Favorites"},{key:"local",label:"Downloaded wallpapers"},{key:"downloads",label:"Download center"},{key:"settings",label:"Settings"}] as const;
const confirmingExit = signal(false);
function App() {
  const win = currentWindow();
  const show = () => {win.show(); win.focus();};
  const canHide = () => !!preferences()?.background_resident_enabled && Library.trayAvailable();
  const drawer = signal(false), expanded = signal<string[]>([]), ready = signal(false);
  const index = computed(() => {
    const byYear = new Map<string,string[]>();
    for(const p of photos()) {
      const year = p.date.slice(0,4), ym = p.date.slice(0,7);
      let months = byYear.get(year);
      if(!months) {months=[];byYear.set(year,months);}
      if(months[months.length-1]!==ym) months.push(ym);
    }
    return [...byYear.entries()];
  });
  function go(key:ReturnType<typeof page>) {page.set(key);drawer.set(false);}
  Application.on("reopen",show);
  effect(() => {
    if(!preferences()?.background_resident_enabled || !Library.trayAvailable()) {Tray.set(null);return;}
    Tray.set({tooltip:t("Wallpaper library"),onClick:show,menu:[
      {label:t("Open main window"),onSelect:show},
      {label:t("Change once now"),onSelect:() => run(() => Library.changeNow())},
      {type:"separator"},{label:t("Exit"),onSelect:() => Application.quit()},
    ]});
  });
  effect(() => {
    if (!active().length) return;
    pollTransfers();
    const timer=setInterval(pollTransfers,250); return () => clearInterval(timer);
  });
  run(async () => {
    await load(); ready.set(true);
    if(photos()[0]) expanded.set([photos()[0].date.slice(0,4)]);
    if(source()) void refresh();
  });
  run(async () => {await loadPreferences(); if(Library.backgroundLaunch() && canHide()) win.hide();});
  effect(() => {
    if (!ready() || !source()) return;
    const timer=setInterval(() => void refresh(),30*60*1000); return () => clearInterval(timer);
  });
  const narrow = () => windowSize().width<800;
  const title = () => page()==="month" ? month() : t(page()==="home" ? "Wallpaper library" : destinations.find(d=>d.key===page())?.label || "Home");
  return <View class="flex-row flex-1 bg-background">
    <If when={!narrow() || drawer()}>
      <Scroll testID="sidebar" class="border-r border-border bg-card" style={{width:narrow()?150:194}}>
        <View class="gap-2 p-3">
          <Each of={destinations}>{entry => <Action id={"nav-"+entry().key} primary={page()===entry().key} label={t(entry().label)} press={() => go(entry().key)}/>}</Each>
          <Text class="pt-3 text-sm font-semibold text-muted-foreground">{t("Archive")}</Text>
          <Each of={index()}>{group => <View class="gap-1">
            <Action id={"year-"+group()[0]} label={(expanded().includes(group()[0])?"▾ ":"▸ ")+group()[0]} press={() => {
              const y=group()[0];expanded.update(years => years.includes(y)?years.filter(v=>v!==y):[...years,y]);
            }}/>
            <If when={expanded().includes(group()[0])}><View class="gap-1 pl-3"><Each of={group()[1]}>{ym =>
              <Action id={"month-"+ym()} label={ym()} primary={page()==="month" && month()===ym()} press={() => {month.set(ym());go("month");}}/>
            }</Each></View></If>
          </View>}</Each>
        </View>
      </Scroll>
    </If>
    <View class="flex-1 gap-3 p-4">
      <View class="flex-row flex-wrap items-center gap-3">
        <If when={narrow()}><Action id="navigation" label={t("Navigation")} press={() => drawer.set(!drawer())}/></If>
        <Text testID="page-title" class="text-2xl font-semibold text-foreground">{title()}</Text>
      </View>
      <If when={!ready()}><Text>{t("Loading wallpaper list...")}</Text></If>
      <If when={ready() && !source() && page()!=="settings"}><Region/></If>
      <If when={page()==="home" || page()==="month" || page()==="favorites"}><Gallery/></If>
      <If when={page()==="local"}><LocalGallery/></If>
      <If when={page()==="downloads"}><DownloadCenter/></If>
      <If when={page()==="settings" && !!preferences()}><Settings/></If>
      <If when={!!status()}><Text testID={applied() ? "applied" : "status"} class="text-sm text-muted-foreground">{status()}</Text></If>
    </View>
    <If when={selected()}>{photo => <Preview photo={photo()}/>}</If>
    <If when={confirmingExit()}><Dialog open class="m-auto rounded-xl bg-card" onOpenChange={({open})=>{if(!open)confirmingExit.set(false);}}>
      <View class="gap-3 p-4"><Text class="font-semibold text-foreground">{t("Exit app prompt")}</Text>
        <View class="flex-row gap-2"><Action label={t("Cancel")} press={()=>confirmingExit.set(false)}/>
          <If when={canHide()}><Action label={t("Minimize to tray")} press={()=>{confirmingExit.set(false);win.hide();}}/></If>
          <Action primary label={t("Exit application")} press={()=>Application.quit()}/>
        </View>
      </View>
    </Dialog></If>
  </View>;
}
mount(() => <Window id="main" title="必应每日壁纸库" closeOnPress={false} onClosePress={()=>{
  if(preferences()?.background_resident_enabled && Library.trayAvailable()) confirmingExit.set(true);
  else Application.quit();
}}>{App}</Window>);
