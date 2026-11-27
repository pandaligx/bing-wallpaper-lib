import { List, signal, Text, View } from "nsh";
import { DatePicker, Progress } from "nsh/base";
import { cancelPending, enqueue, jobs, month, pauseTransfer, photos, saved, transfers } from "../library";
import type { Job } from "../library";
import { t } from "../preferences";
import { Action } from "./Action";

const range = signal<[string|null,string|null]>([null,null]);
function Task(props:{job:Job}) {
  const job = () => props.job;
  const transfer = () => transfers().find(tr => tr.date === job().photo.date && tr.state === "active");
  return <View class="gap-1 border-b border-border py-2">
    <View class="flex-row flex-wrap items-center justify-between gap-2">
      <Text class="font-semibold text-foreground">{job().photo.headline || job().photo.date}</Text>
      <Text class="text-muted-foreground">{job().photo.date} · {t(job().state)}</Text>
    </View>
    <If when={job().path || job().error}><Text class="text-sm text-muted-foreground">{job().error || job().path}</Text></If>
    <If when={transfer()}>{tr => <View class="flex-row items-center gap-2">
      <Progress class="flex-1" value={100*tr().received/(tr().total||1)} indeterminate={!tr().total}
       accessibilityLabel={t("Download progress")} trackClass="h-2 rounded bg-muted" indicatorClass="h-2 rounded bg-primary"/>
      <Text>{(tr().received/1048576).toFixed(1)} / {tr().total ? (tr().total!/1048576).toFixed(1) : "?"} MiB · {Math.round(tr().speed/1024)} KiB/s</Text>
      <Action label={t("Pause")} press={() => void pauseTransfer(tr().id)}/>
    </View>}</If>
    <If when={job().state === "Waiting"}><Action label={t("Cancel")} press={() => cancelPending(job().photo.date)}/></If>
    <If when={job().state === "Failed" || job().state === "Pause"}><Action label={t("Retry")} press={() => enqueue([job().photo],job().apply)}/></If>
  </View>;
}
export function DownloadCenter() {
  const min = () => photos()[photos().length-1]?.date ?? "";
  const max = () => photos()[0]?.date ?? "";
  const currentMonth = () => month() || max().slice(0,7);
  const span = () => range();
  const valid = () => !!(span()[0] && span()[1] && span()[0]! >= min() && span()[1]! <= max() && span()[0]! <= span()[1]!);
  return <View class="flex-1 gap-3">
    <View class="flex-row flex-wrap gap-2">
      <Action id="batch-all" label={t("All history")} disabled={!max()} press={() => enqueue(photos())}/>
      <Action id="batch-month" label={t("Current month")+" · "+currentMonth()} disabled={!max()} press={() => enqueue(photos().filter(p => p.date.startsWith(currentMonth())))}/>
      <Action id="batch-favorites" label={t("Favorites")+" · "+saved().length} disabled={!saved().length} press={() => {const dates = new Set(saved()); enqueue(photos().filter(p => dates.has(p.date)));}}/>
    </View>
    <View class="flex-row flex-wrap items-center gap-2">
      <DatePicker range value={range()} disabled={!max()} disabledMatcher={date => date < min() || date > max()}
        yearRange={[Number(min().slice(0,4)),Number(max().slice(0,4))+1]} accessibilityLabel={t("Select a date range")}
        onChange={({value}) => range.set(Array.isArray(value) ? value : [null,null])}
        trigger={<Text class="rounded-lg border border-border p-2 text-foreground">{(span()[0]||min())+" — "+(span()[1]||max())}</Text>}/>
      <Action id="batch-range" label={t("Download date range")} disabled={!valid()} press={() => enqueue(photos().filter(p => p.date >= span()[0]! && p.date <= span()[1]!))}/>
    </View>
    <Text class="text-sm text-muted-foreground">{jobs().length ? [ "Waiting","Active","Done","Skipped","Failed","Pause" ].map(state => t(state)+" "+jobs().filter(j => j.state === state).length).join("  ·  ") : t("Empty")}</Text>
    <List each={jobs()} keyExtractor={j => j.photo.date} estimatedItemSize={75} class="flex-1">{j => <Task job={j()}/>}</List>
  </View>;
}
