import { Text, View } from "nsh";
import { Select, Switch } from "nsh/base";

export type Choice = { title:string; value:string; disabled?:boolean };
export function ChoiceField(props:{id?:string; label:string; items:Choice[]; value:string; choose:(v:string)=>void}) {
  return <View class="gap-1"><Text class="text-sm text-muted-foreground">{props.label}</Text>
    <Select testID={props.id} items={props.items} value={props.value} accessibilityLabel={props.label}
      onChange={({value}) => {if(value !== null) props.choose(value);}} popupClass="rounded-lg border border-border bg-card p-1"
      itemClass="rounded px-3 py-2 hover:bg-accent"
      trigger={<Text class="rounded-lg border border-border px-3 py-2 text-foreground">{props.items.find(i => i.value === props.value)?.title ?? props.value}</Text>}
      renderItem={item => <Text class="text-foreground">{item.title}</Text>}/>
  </View>;
}
export function Toggle(props:{id?:string;label:string; value:boolean; disabled?:boolean; change:(v:boolean)=>void}) {
  return <View class="flex-row items-center gap-3">
    <Switch testID={props.id} checked={props.value} disabled={props.disabled} accessibilityLabel={props.label}
      onChange={({checked}) => props.change(checked)} trackClass="h-6 w-10 rounded-full bg-muted checked:bg-primary"
      thumbClass="h-5 w-5 rounded-full bg-foreground checked:translate-x-4 checked:bg-primary-foreground"/>
    <Text class="flex-1 text-foreground">{props.label}</Text>
  </View>;
}
