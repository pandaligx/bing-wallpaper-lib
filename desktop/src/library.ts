import { signal } from "nsh";
import { Library } from "wallpaper-native";

export interface Photo {
  date: string;
  headline: string | null;
  description: string;
  thumbnail: string;
  preview: string;
}

export const photos = signal<Photo[]>([]);
export const saved = signal<string[]>([]);
export const selected = signal<Photo | null>(null);
export const status = signal("");
export const source = signal("");
export const active = signal<string[]>([]);
interface Transfer { id: string; date: string; received: number; total: number | null; speed: number; state: string }
export const transfers = signal<Transfer[]>([]);

export function pollTransfers() {
  try { transfers.set(JSON.parse(Library.progress())); }
  catch (error) { status.set(String(error)); }
}
export async function pauseTransfer(id: string) {
  try { await Library.pause(id); }
  catch (error) { status.set(String(error)); }
}

export async function load() {
  photos.set(JSON.parse(await Library.catalog()));
  saved.set(JSON.parse(Library.favorites()));
  source.set(Library.source());
}

export async function refresh() {
  try {
    const warnings = await Library.refresh();
    photos.set(JSON.parse(await Library.catalog()));
    if (warnings) status.set(warnings);
  } catch (error) { status.set(String(error)); }
}

export function setSource(value: string) {
  try {
    Library.setSource(value);
    source.set(Library.source());
    if (source()) void refresh();
  } catch (error) { status.set(String(error)); }
}

export function favorite(photo: Photo) {
  try { saved.set(JSON.parse(Library.favorite(photo.date))); }
  catch (error) { status.set(String(error)); }
}

export async function download(photo: Photo, apply: boolean) {
  if (active().includes(photo.date)) return;
  active.update(items => [...items, photo.date]);
  try {
    const path = await Library.download(photo.date, apply);
    status.set(apply ? "已设置桌面壁纸" : `已保存到 ${path}`);
  } catch (error) { status.set(String(error)); }
  finally { active.update(items => items.filter(date => date !== photo.date)); }
}
