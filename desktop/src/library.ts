import { computed, signal } from "nsh";
import { Library } from "wallpaper-native";
import { Application } from "nsh/desktop";
import { applied, isError, notify, reportError, t } from "./preferences";
export { status } from "./preferences";

export interface Photo {
  date: string;
  headline: string | null;
  description: string;
  thumbnail: string;
  preview: string;
}
export const photos = signal<Photo[]>([]);
export const localFiles = signal<string[]>([]);
let scanVersion = 0;
export async function scanLocalFiles() {
  const version = ++scanVersion;
  const list: string[] = JSON.parse(await Library.localFiles());
  if (version === scanVersion) localFiles.set(list);
}
export const saved = signal<string[]>([]);
export const selected = signal<Photo | null>(null);
export const source = signal("");
export const sourcePreference = signal("auto");
export const page = signal<"home" | "month" | "favorites" | "local" | "downloads" | "settings">("home");
export const month = signal("");
export const entries = computed(() => {
  if (page() === "month") return photos().filter((p) => p.date.startsWith(month()));
  if (page() === "favorites") {
    const dates = new Set(saved());
    return photos().filter((p) => dates.has(p.date));
  }
  return photos();
});
interface Update {
  state: "current" | "available" | "staged";
  provider?: string;
  version?: string;
  minimum?: string;
  bytes?: number;
  path?: string;
}
export const update = signal<Update | null>(null);
export const checking = signal(false);
export const downloadingUpdate = signal(false);
export async function checkUpdate(download = false, automatic = false) {
  if (checking() || !source()) return;
  const provider = source();
  checking.set(true);
  downloadingUpdate.set(download);
  try {
    const result: Update = JSON.parse(await Library.checkUpdate(download));
    if (source() !== provider) return;
    if (result.state === "current") {
      if (!automatic) notify(t("Already up to date") + " · " + provider);
    } else update.set(result);
  } catch (error) {
    if (!automatic) {
      update.set(null);
      reportError(error, () => void checkUpdate(download));
    }
  } finally {
    downloadingUpdate.set(false);
    checking.set(false);
  }
}
export async function installUpdate() {
  const item = update();
  if (!item?.version || item.state !== "staged" || checking()) return;
  checking.set(true);
  try {
    await Library.installUpdate(item.version);
    Application.quit();
  } catch (error) {
    reportError(error);
    update.set(null);
  } finally {
    checking.set(false);
  }
}
interface Transfer {
  id: string;
  update?: boolean;
  date: string;
  received: number;
  total: number | null;
  speed: number;
  state: string;
}
export const transfers = signal<Transfer[]>([]);

export function pollTransfers() {
  try {
    transfers.set(JSON.parse(Library.progress()));
  } catch (error) {
    reportError(error);
  }
}
let lastCatalogRevision: number | undefined;
async function loadCatalog() {
  const revision = Library.catalogRevision();
  if (lastCatalogRevision !== undefined && revision === lastCatalogRevision) return;
  const next: Photo[] = JSON.parse(await Library.catalog());
  if (lastCatalogRevision === undefined || revision > lastCatalogRevision) {
    lastCatalogRevision = revision;
    photos.set(next);
  }
}
export async function load() {
  await loadCatalog();
  saved.set(JSON.parse(Library.favorites()));
  source.set(Library.source());
  sourcePreference.set(Library.regionPreference());
}
let refreshing = false;
export async function refresh() {
  if (refreshing) return;
  const requested = source();
  refreshing = true;
  try {
    const warnings = await Library.refresh();
    await loadCatalog();
    if (warnings) reportError(warnings, () => void refresh());
  } catch (error) {
    reportError(error, () => void refresh());
  } finally {
    refreshing = false;
    if (source() && source() !== requested) void refresh();
  }
}
export function setSource(value: string) {
  try {
    Library.setSource(value);
    sourcePreference.set(value);
    notify("");
    source.set(Library.source());
    update.set(null);
    if (source()) void refresh();
  } catch (error) {
    reportError(error);
  }
}
export function favorite(photo: Photo) {
  try {
    saved.set(JSON.parse(Library.favorite(photo.date)));
  } catch (error) {
    reportError(error);
  }
}

export interface Job {
  photo: Photo;
  state: "Waiting" | "Active" | "Done" | "Skipped" | "Failed" | "Pause" | "Cancelled";
  apply: boolean;
  error?: string;
  path?: string;
}
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
  writing = (async () => {
    while (dirty) {
      dirty = false;
      const rows = jobs().map(({ photo, ...job }) => ({ date: photo.date, ...job }));
      await Library.saveDownloadHistory(JSON.stringify(rows));
    }
  })()
    .catch((error) => reportError(error))
    .finally(() => {
      writing = null;
    });
  return writing;
}
function saveHistory() {
  dirty = true;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => void flushHistory(), 400);
}
export async function restoreHistory() {
  const data: (Omit<Job, "photo"> & { date: string })[] = JSON.parse(await Library.downloadHistory());
  const byDate = new Map(photos().map((p) => [p.date, p]));
  const existing = new Set(jobs().map((j) => j.photo.date));
  const restored: Job[] = [];
  for (const { date, ...j } of data) {
    const photo = byDate.get(date);
    if (photo && !existing.has(date)) {
      restored.push({
        ...j,
        photo,
        state: j.state === "Waiting" || j.state === "Active" ? "Pause" : j.state,
      });
      existing.add(date);
    }
  }
  jobs.update((current) => [...current, ...restored]);
}
function change(date: string, patch: Partial<Job>) {
  jobs.update((list) => list.map((j) => (j.photo.date === date ? { ...j, ...patch } : j)));
  saveHistory();
}
async function worker() {
  workers++;
  try {
    while (pending.length) {
      const job = pending.shift()!;
      const date = job.photo.date;
      change(date, { state: "Active" });
      active.update((dates) => [...dates, date]);
      let reserved = false;
      try {
        Library.beginDownload(date);
        reserved = true;
        const result = JSON.parse(await Library.download(date, job.apply));
        change(date, { state: result.reused ? "Skipped" : "Done", path: result.path });
        if (job.apply) {
          applied.set(true);
          notify(t("Applied"));
        } else if (!pending.length && !isError()) notify(result.path);
      } catch (err) {
        if (String(err).includes("queue:paused")) change(date, { state: "Pause" });
        else if (String(err).includes("queue:cancelled")) change(date, { state: "Cancelled" });
        else {
          change(date, { state: "Failed", error: String(err) });
          reportError(err);
        }
      } finally {
        if (reserved) Library.finishDownload(date);
        active.update((dates) => dates.filter((d) => d !== date));
      }
    }
  } finally {
    workers--;
    if (workers === 0) void scanLocalFiles().catch(reportError);
  }
}
export function enqueue(items: Photo[], apply = false) {
  if (apply) applied.set(false);
  const busy = new Set(
    jobs()
      .filter((j) => j.state === "Active" || j.state === "Waiting")
      .map((j) => j.photo.date),
  );
  const add: Job[] = [];
  for (const photo of items)
    if (!busy.has(photo.date) && !stopping.has(photo.date)) {
      busy.add(photo.date);
      add.push({ photo, apply, state: "Waiting" });
    }
  if (!add.length) return;
  notify("");
  const added = new Set(add.map((j) => j.photo.date));
  jobs.update((list) => [...add, ...list.filter((j) => !added.has(j.photo.date))]);
  saveHistory();
  pending.push(...add);
  while (workers < 4 && pending.length) void worker();
}
const stopping = new Set<string>();
export async function stopJob(job: Job, cancel: boolean) {
  const date = job.photo.date;
  if (stopping.has(date)) return;
  stopping.add(date);
  const index = pending.findIndex((queued) => queued.photo.date === date);
  if (index >= 0) {
    pending.splice(index, 1);
  }
  try {
    await Library.stopDownload(date, cancel);
    if (index >= 0 || job.state === "Pause") change(date, { state: cancel ? "Cancelled" : "Pause" });
  } catch (error) {
    if (index >= 0) change(date, { state: "Failed", error: String(error) });
    reportError(error);
  } finally {
    stopping.delete(date);
  }
}
export function pauseJobs(items: Job[]) {
  const dates = new Set(items.map((job) => job.photo.date));
  const waiting = new Set(pending.filter((job) => dates.has(job.photo.date)).map((job) => job.photo.date));
  if (waiting.size) {
    const rest = pending.filter((job) => !waiting.has(job.photo.date));
    pending.splice(0, pending.length, ...rest);
    jobs.update((rows) =>
      rows.map((job) => (waiting.has(job.photo.date) ? { ...job, state: "Pause" } : job)),
    );
    saveHistory();
  }
  for (const job of items) if (active().includes(job.photo.date)) void stopJob(job, false);
}
export function removeJobs(dates: string[]) {
  const remove = new Set(dates);
  jobs.update((rows) =>
    rows.filter(
      (job) =>
        !remove.has(job.photo.date) ||
        job.state === "Active" ||
        job.state === "Waiting" ||
        stopping.has(job.photo.date),
    ),
  );
  saveHistory();
}
export function download(photo: Photo, apply: boolean) {
  enqueue([photo], apply);
}
