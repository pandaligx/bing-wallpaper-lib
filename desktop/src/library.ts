import { signal } from "nsh";
import { Library } from "wallpaper-native";
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
export async function checkUpdate(download = false) {
  try { status.set(await Library.checkUpdate(download)); }
  catch (error) { status.set(String(error)); }
}
interface Transfer { id: string; date: string; received: number; total: number | null; speed: number; state: string }
export const transfers = signal<Transfer[]>([]);
export function pollTransfers() {
  try { transfers.set(JSON.parse(Library.progress())); } catch (error) { status.set(String(error)); }
}
export async function pauseTransfer(id: string) {
  try { await Library.pause(id); } catch (error) { status.set(String(error)); }
}
export async function load() {
  photos.set(JSON.parse(await Library.catalog()));
  saved.set(JSON.parse(Library.favorites()));
  source.set(Library.source());
}
let refreshing = false;
export async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    const warnings = await Library.refresh();
    photos.set(JSON.parse(await Library.catalog()));
    if (warnings) status.set(warnings);
  } catch (error) { status.set(String(error)); }
  finally { refreshing = false; }
}
export function setSource(value: string) {
  try { Library.setSource(value); source.set(Library.source()); if (source()) void refresh(); }
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
function change(date: string, patch: Partial<Job>) {
  jobs.update(list => list.map(j => j.photo.date === date ? {...j,...patch} : j));
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
        change(date,{state:"Failed",error:String(err)});
        status.set(String(err));
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
  pending.push(...add);
  while (workers < 4 && pending.length) void worker();
}
export function cancelPending(date: string) {
  const index = pending.findIndex(j => j.photo.date === date);
  if (index >= 0) { pending.splice(index,1); change(date,{state:"Pause"}); }
}
export function download(photo: Photo, apply: boolean) { enqueue([photo],apply); }
