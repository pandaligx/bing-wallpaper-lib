import { signal, Text, View } from "nsh";
import { Select } from "nsh/base";
import { Library } from "wallpaper-native";
import { status } from "../library";

type Choice = { title: string; value: string; disabled?: boolean };
type Preferences = {download_resolution: string; wallpaper_target: {type: string; monitor_id?: string}};
const resolutions = [
  {value:"original", title:"原图 UHD"}, {value:"four_k",title:"4K · 3840×2160"},
  {value:"two_k",title:"2K · 2560×1440"}, {value:"one_k",title:"1K · 1920×1080"},
];

function ChoiceField(props: {label: string; items: Choice[]; value: string; choose: (value: string) => void}) {
  return <View class="gap-1">
    <Text class="text-sm text-muted-foreground">{props.label}</Text>
    <Select items={props.items} value={props.value} accessibilityLabel={props.label}
      onChange={({value}) => { if (value !== null) props.choose(value); }} popupClass="rounded-lg border border-border bg-card p-1"
      itemClass="rounded px-3 py-2 hover:bg-accent"
      trigger={<Text class="rounded-lg border border-border px-3 py-2 text-foreground">{props.items.find(item => item.value === props.value)?.title ?? props.value}</Text>}
      renderItem={item => <Text class="text-foreground">{item.title}</Text>} />
  </View>;
}

export function WallpaperOptions() {
  const settings = signal<Preferences | null>(null);
  const monitors = signal<Choice[]>([{value:"",title:"全部显示器"}]);
  void Library.preferences().then(raw => settings.set(JSON.parse(raw))).catch(error => status.set(String(error)));
  try {
    const list: {id: string; label: string}[] = JSON.parse(Library.monitors());
    monitors.set([{value:"",title:"全部显示器"}, ...list.map(m => ({value:m.id,title:m.label}))]);
  } catch (error) { status.set(String(error)); }
  async function change(patch: object) {
    try { settings.set(JSON.parse(await Library.configure(JSON.stringify(patch)))); }
    catch (error) { status.set(String(error)); }
  }
  return <If when={settings()}>{s => <View class="flex-row flex-wrap gap-3">
    <ChoiceField label="下载分辨率" value={s().download_resolution} items={resolutions}
      choose={value => void change({download_resolution:value})}/>
    <ChoiceField label="壁纸显示器" value={s().wallpaper_target.monitor_id ?? ""}
      items={monitors().some(m => m.value === (s().wallpaper_target.monitor_id ?? "")) ? monitors() :
        [...monitors(), {value:s().wallpaper_target.monitor_id!, title:"已断开的显示器", disabled:true}]}
      choose={value => void change({wallpaper_target:value ? {type:"monitor",monitor_id:value} : {type:"all"}})}/>
  </View>}</If>;
}
