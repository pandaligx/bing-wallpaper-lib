import { signal } from "nsh";
import { Library } from "wallpaper-native";
import { Application } from "nsh/desktop";
import { applied, status, t } from "./preferences";
export { status } from "./preferences";

export interface Photo { date: string; headline: string | null; description: string; thumbnail: string; preview: string }
export const photos = signal<Photo[]>([]);
export const saved = signal<string[]>([]);
export const selected = signal<Photo | null>(null);
export const source = signal("");
export const page = signal<"home" | "month" | "favorites" | "local" | "downloads" | "settings">("home");
export const month = signal("");
export function entries() {
  if (page() === "month") return photos().filter(p => p.date.startsWith(month()));
  if (page() === "favorites") { const dates = new Set(saved()); return photos().filter(p => dates.has(p.date)); }
  return photos();
}
interface Update {state:"current"|"available"|"staged";provider?:string;version?:string;minimum?:string;bytes?:number;path?:string}
export const update = signal<Update|null>(null);
export const checking = signal(false);
export async function checkUpdate(download = false, automatic = false) {
  if(checking() || !source()) return;
  const provider=source();
  checking.set(true);
  try {
    const result:Update=JSON.parse(await Library.checkUpdate(download));
    if(source()!==provider) return;
    if(result.state==="current") {if(!automatic) status.set(t("Already up to date")+" · "+provider);}
    else update.set(result);
  } catch (error) {if(!automatic) status.set(String(error));}
  finally {checking.set(false);}
}
export async function installUpdate() {
  const item=update();
  if(!item?.version || item.state!=="staged" || checking()) return;
  checking.set(true);
  try {await Library.installUpdate(item.version); Application.quit();}
  catch(error){status.set(String(error));update.set(null);}
  finally {checking.set(false);}
}
interface Transfer { id: string; date: string; received: number; total: number | null; speed: number; state: string }
export const transfers = signal<Transfer[]>([]);
const pausedDates=new Set<string>();
export function pollTransfers() {
  try { transfers.set(JSON.parse(Library.progress())); } catch (error) { status.set(String(error)); }
}
export async function pauseTransfer(id: string) {
  const date=transfers().find(item=>item.id===id)?.date;
  if(date) pausedDates.add(date);
  try { await Library.pause(id); } catch (error) {if(date) pausedDates.delete(date);status.set(String(error));}
}
export async function load() {
  photos.set(JSON.parse(await Library.catalog()));
  saved.set(JSON.parse(Library.favorites()));
  source.set(Library.source());
}
let refreshing = false;
export async function refresh() {
  if (refreshing) return;
  const requested=source();
  refreshing = true;
  try {
    const warnings = await Library.refresh();
    photos.set(JSON.parse(await Library.catalog()));
    if (warnings) status.set(warnings);
  } catch (error) { status.set(String(error)); }
  finally { refreshing = false; if(source() && source()!==requested) void refresh(); }
}
export function setSource(value: string) {
  try { Library.setSource(value); source.set(Library.source()); update.set(null); if (source()) void refresh(); }
  catch (error) { status.set(String(error)); }
}
export function favorite(photo: Photo) {
  try { saved.set(JSON.parse(Library.favorite(photo.date))); } catch (error) { status.set(String(error)); }
}

export interface Job { photo: Photo; state: "Waiting" | "Active" | "Done" | "Skipped" | "Failed" | "Pause"; apply: boolean; error?: string; path?: string }
export const jobs = signal<Job[]>([]);
export const active = signal<string[]>([]);
let workers = 0;
const pending: Job[] = [];
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let dirty = false;
let writing: Promise<void> | null = null;
export function flushHistory(): Promise<void> {
  if (saveTimer) clearTimeout(saveTimer);
  if (writing) return writing;
  writing=(async () => {
    while(dirty) {
      dirty=false;
      const rows=jobs().map(({photo,...job})=>({date:photo.date,...job}));
      await Library.saveDownloadHistory(JSON.stringify(rows));
    }
  })().catch(error=>status.set(String(error))).finally(()=>{writing=null;});
  return writing;
}
function saveHistory() {
  dirty=true;
  if(saveTimer) clearTimeout(saveTimer);
  saveTimer=setTimeout(()=>void flushHistory(),400);
}
export async function restoreHistory() {
  const data: (Omit<Job,"photo"> & {date:string})[] = JSON.parse(await Library.downloadHistory());
  const byDate = new Map(photos().map(p=>[p.date,p]));
  const existing = new Set(jobs().map(j=>j.photo.date));
  const restored:Job[]=[];
  for(const {date,...j} of data) {
    const photo=byDate.get(date);
    if(photo && !existing.has(date)) {restored.push({...j,photo,state:j.state==="Waiting"||j.state==="Active"?"Pause":j.state});existing.add(date);}
  }
  jobs.update(current=>[...current,...restored]);
}
function change(date: string, patch: Partial<Job>) {
  jobs.update(list => list.map(j => j.photo.date === date ? {...j,...patch} : j));
  saveHistory();
}
async function worker() {
  workers++;
  try {
    while (pending.length) {
      const job = pending.shift()!;
      const date = job.photo.date;
      change(date,{state:"Active"});
      active.update(dates => [...dates,date]);
      try {
        const result = JSON.parse(await Library.download(date,job.apply));
        change(date,{state:result.reused ? "Skipped" : "Done",path:result.path});
        if (job.apply) { applied.set(true); status.set(t("Applied")); }
        else if (!pending.length) status.set(result.path);
      } catch (err) {
        if(pausedDates.delete(date)) change(date,{state:"Pause"});
        else {change(date,{state:"Failed",error:String(err)});status.set(String(err));}
      } finally { active.update(dates => dates.filter(d => d !== date)); }
    }
  } finally { workers--; }
}
export function enqueue(items: Photo[], apply = false) {
  if (apply) applied.set(false);
  const busy = new Set(jobs().filter(j => j.state === "Active" || j.state === "Waiting").map(j => j.photo.date));
  const add: Job[] = [];
  for (const photo of items) if (!busy.has(photo.date)) { busy.add(photo.date); add.push({photo, apply, state:"Waiting"}); }
  const added = new Set(add.map(j => j.photo.date));
  jobs.update(list => [...add, ...list.filter(j => !added.has(j.photo.date))]);
  saveHistory();
  pending.push(...add);
  while (workers < 4 && pending.length) void worker();
}
export function cancelPending(date: string) {
  const index = pending.findIndex(j => j.photo.date === date);
  if (index >= 0) { pending.splice(index,1); change(date,{state:"Pause"}); }
}
export function download(photo: Photo, apply: boolean) { enqueue([photo],apply); }
