import { Scroll, signal, Text, View } from "nsh";
import { Input } from "nsh/base";
import { Application } from "nsh/desktop";
import { Library } from "wallpaper-native";
import { checkUpdate, load, refresh, setSource, source } from "../library";
import { configure, preferences, run, status, t } from "../preferences";
import type { Preferences } from "../preferences";
import { Action } from "./Action";
import { ChoiceField, Toggle } from "./WallpaperOptions";

export function Region() {
  return <View class="gap-2"><Text class="font-semibold text-foreground">{t("Region")}{source() ? " · "+source() : ""}</Text>
    <View class="flex-row flex-wrap gap-2"><Action label={t("Follow system")} press={() => setSource("auto")}/>
      <Action id="region-cn" label="GitCode · CN" press={() => setSource("cn")}/>
      <Action id="region-other" label="GitHub" press={() => setSource("international")}/></View>
  </View>;
}
const languages = [["system","Follow system"],["english","English"],["simplified_chinese","简体中文"],["japanese","日本語"],["korean","한국어"],["russian","Русский"],["french","Français"]];
const sources = [["latest","Latest daily wallpaper"],["random_all","Random from all history"],["random_favorites","Random from favorites"]];
function choices(pairs:string[][]) {return pairs.map(([value,title]) => ({value,title:t(title)}));}
function Heading(props:{label:string}) {return <Text class="border-b border-border pb-2 text-lg font-semibold text-foreground">{props.label}</Text>;}
const inputClass = "min-w-24 rounded-lg border border-border bg-card px-3 py-2 text-foreground";

async function startup(enabled:boolean) {
  const sys = (v:boolean) => Library.hostOs()==="macos" ? Application.setLaunchAtLogin(v) : Library.configureStartup(v);
  await sys(enabled);
  try {await configure({startup_enabled:enabled});} catch(error) {await sys(!enabled); throw error;}
}
export function Settings() {
  const s = () => preferences()!;
  const path = signal(""), time = signal(""), interval = signal("");
  const displays = signal<{id:string;label:string}[]>([]);
  function getDisplays() {try {displays.set(JSON.parse(Library.monitors()));} catch(e){status.set(String(e));}}
  getDisplays();
  run(async () => path.set(await Library.downloadFolder()));
  const clock = () => String(s().auto_wallpaper_hour).padStart(2,"0")+":"+String(s().auto_wallpaper_minute).padStart(2,"0");
  const save = (patch:Partial<Preferences>, scheduled=false) => run(() => configure(patch,scheduled));
  const monitorOptions = () => {
    const options = [{value:"",title:t("Sync all displays"),disabled:false},...displays().map(m => ({value:m.id,title:m.label,disabled:false}))];
    const current = s().wallpaper_target.monitor_id;
    if(current && !options.some(m => m.value === current)) options.push({value:current,title:t("Disconnected"),disabled:true});
    return options;
  };
  return <Scroll testID="settings-content" class="flex-1"><View class="gap-4 pb-5 pr-2">
    <Heading label={t("Appearance")}/>
    <View class="flex-row flex-wrap gap-4">
      <ChoiceField id="theme" label={t("Appearance")} value={s().theme_preference} items={choices([["system","System"],["light","Light"],["dark","Dark"]])} choose={v => save({theme_preference:v as Preferences["theme_preference"]})}/>
      <ChoiceField id="language" label={t("Language")} value={s().language} items={choices(languages)} choose={v => save({language:v})}/>
    </View>
    <Heading label={t("Wallpaper download folder")}/>
    <Input testID="folder-path" accessibilityLabel={t("Wallpaper download folder")} class={inputClass} value={path()} onChange={({value})=>path.set(value)}/>
    <View class="flex-row flex-wrap gap-2">
      <Action label={t("Choose and save")} press={() => run(async () => {const result=await Library.chooseFolder(); if(result) {await configure({download_dir:result});path.set(result);}})}/>
      <Action label={t("Save")} press={() => run(async () => {await configure({download_dir:path().trim()||null});path.set(await Library.downloadFolder());})}/>
      <Action label={t("Default")} press={() => run(async () => {await configure({download_dir:null}); path.set(await Library.downloadFolder());})}/>
      <Action label={t("Open folder")} press={() => run(() => Library.openLocation("folder"))}/>
    </View>
    <ChoiceField id="resolution" label={t("Global resolution")} value={s().download_resolution}
      items={choices([["original",t("Original")+" UHD"],["four_k","4K · 3840×2160"],["two_k","2K · 2560×1440"],["one_k","1K · 1920×1080"]])} choose={v => save({download_resolution:v})}/>
    <Heading label={t("Multi-monitor wallpaper")}/>
    <View class="flex-row flex-wrap items-center gap-2">
      <ChoiceField label={t("Multi-monitor wallpaper")} items={monitorOptions()} value={s().wallpaper_target.monitor_id??""}
        choose={v => save({wallpaper_target:v ? {type:"monitor",monitor_id:v} : {type:"all"}})}/>
      <Action label={t("Refresh")} press={getDisplays}/>
    </View>
    <Heading label={t("Automatic wallpaper")}/>
    <Toggle label={t("Run in background / show tray icon")} value={s().background_resident_enabled} change={v => save({background_resident_enabled:v})}/>
    <Toggle label={t("Startup")} value={s().startup_enabled} disabled={s().periodic_task_enabled} change={v => run(() => startup(v))}/>
    <Toggle id="daily-enabled" label={t("Change wallpaper daily")} value={s().auto_wallpaper_enabled} change={v => save({auto_wallpaper_enabled:v},true)}/>
    <ChoiceField label={t("Wallpaper source")} items={choices(sources)} value={s().auto_wallpaper_source} choose={v => save({auto_wallpaper_source:v},true)}/>
    <View class="flex-row flex-wrap items-center gap-2">
      <Text>{t("Execution time")}</Text>
      <Input class={inputClass} value={time()||clock()} accessibilityLabel={t("Execution time")} onChange={({value})=>time.set(value)}/>
      <Action label={t("Save")} disabled={!/^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(time())} press={() => {
        const [hour,minute]=time().split(":").map(Number); save({auto_wallpaper_hour:hour,auto_wallpaper_minute:minute},true);
      }}/>
      <Action label={t("Change once now")} press={() => run(async()=>{await Library.changeNow();status.set(t("Applied"));})}/>
    </View>
    <Toggle label={t("Exit after automatic wallpaper")} value={s().auto_wallpaper_exit_after_done} change={v => save({auto_wallpaper_exit_after_done:v})}/>
    <Toggle id="periodic-enabled" label={t("Enable periodic task scheduler")} value={s().periodic_task_enabled} change={v => run(async () => {
      const disableStartup = v && s().startup_enabled;
      if(disableStartup) await startup(false);
      try {await configure({periodic_task_enabled:v},true);} catch(e){if(disableStartup) await startup(true); throw e;}
    })}/>
    <Toggle label={t("Use latest Bing wallpaper for the first run each day")} value={s().periodic_daily_first_latest} change={v => save({periodic_daily_first_latest:v},true)}/>
    <ChoiceField label={t("Later wallpaper source")} value={s().periodic_wallpaper_source} items={choices(sources.slice(1))} choose={v => save({periodic_wallpaper_source:v},true)}/>
    <View class="flex-row flex-wrap items-center gap-2"><Text>{t("Repeat interval")} (1–1439 min)</Text>
      <Input class={inputClass} accessibilityLabel={t("Repeat interval")} value={interval() || String(s().periodic_interval_minutes)} onChange={({value})=>interval.set(value)}/>
      <Action label={t("Save")} disabled={!/^\d{1,4}$/.test(interval()) || Number(interval())<1 || Number(interval())>1439}
        press={() => save({periodic_interval_minutes:Number(interval())},true)}/>
    </View>
    <Heading label={t("Maintenance")}/><Region/>
    <View class="flex-row flex-wrap gap-2">
      <Action label={t("Refresh wallpaper list")} disabled={!source()} press={() => void refresh()}/>
      <Action label={t("Clear wallpaper cache")} press={() => run(async()=>{await Library.clearThumbnailCache();await load();if(source())await refresh();})}/>
      <Action label={t("Check for updates")} disabled={!source()} press={() => void checkUpdate()}/>
      <Action label={t("Update now")} disabled={!source()} press={() => void checkUpdate(true)}/>
      <Action label={t("View release notes")} disabled={!source()} press={() => run(() => Library.openLocation("releases"))}/>
    </View>
    <Heading label={t("About")}/><Text class="text-muted-foreground">{t("Version")} {Library.appVersion()} · © 2023-2026 小南瓜</Text>
    <Action label={t("Project home")} disabled={!source()} press={() => run(() => Library.openLocation("project"))}/>
  </View></Scroll>;
}
