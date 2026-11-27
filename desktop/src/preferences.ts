import { localeSettings, setColorScheme, signal } from "nsh";
import { setLocale } from "nsh/i18n";
import { Application } from "nsh/desktop";
import { Library } from "wallpaper-native";

export interface Preferences {
  language: string;
  theme_preference: "system" | "light" | "dark";
  download_dir: string | null;
  download_resolution: string;
  wallpaper_target: { type: string; monitor_id?: string };
  startup_enabled: boolean;
  background_resident_enabled: boolean;
  auto_wallpaper_enabled: boolean;
  auto_wallpaper_source: string;
  auto_wallpaper_hour: number;
  auto_wallpaper_minute: number;
  auto_wallpaper_exit_after_done: boolean;
  last_auto_wallpaper_date?: string | null;
  periodic_task_enabled: boolean;
  periodic_interval_minutes: number;
  periodic_daily_first_latest: boolean;
  periodic_wallpaper_source: string;
}

export const preferences = signal<Preferences | null>(null);
export const status = signal("");
export const applied = signal(false);
export const canApply = signal(false);
const labels = signal<Record<string, string>>({});
const keys =
  "Home|Settings|Wallpaper library|Downloaded wallpapers|Favorites|Download center|Archive|Navigation|Appearance|Language|Follow system|Light|Dark|System|Default|Original|Wallpaper download folder|Choose and save|Open folder|Global resolution|Multi-monitor wallpaper|Sync all displays|No display detected|Automatic wallpaper|Startup|Run in background / show tray icon|Change wallpaper daily|Exit after automatic wallpaper|Wallpaper source|Latest daily wallpaper|Random from all history|Random from favorites|Execution time|Change once now|Enable periodic task scheduler|Use latest Bing wallpaper for the first run each day|Later wallpaper source|Repeat interval|Periodic interval range hint|Maintenance|Clear wallpaper cache|Check for updates|View release notes|About|Version|Copyright|Project home|Download|Set as wallpaper|Preview|Favorite|Cancel favorite|Refresh|Loading wallpaper list...|No favorite wallpapers yet|No downloaded wallpapers yet|Delete selected|Delete|Select all|Clear selection|Refresh wallpaper list|Back to top|Download by date range|Download date range|All history|Current month|Select a date range|Download progress|Failed|Skipped|Update now|Already up to date|Update available|Later|Open main window|Exit|Exit application|Minimize to tray|Exit app prompt".split(
    "|",
  );
keys.push("Speed", "Remaining");
const extras: Record<string, string[]> = {
  Display: ["显示器", "ディスプレイ", "디스플레이", "Монитор", "Écran"],
  Close: ["关闭", "閉じる", "닫기", "Закрыть", "Fermer"],
  Previous: ["上一张", "前へ", "이전", "Назад", "Précédent"],
  Next: ["下一张", "次へ", "다음", "Далее", "Suivant"],
  Pause: ["暂停", "一時停止", "일시 정지", "Пауза", "Pause"],
  Retry: ["重试", "再試行", "재시도", "Повторить", "Réessayer"],
  Save: ["保存", "保存", "저장", "Сохранить", "Enregistrer"],
  Cancel: ["取消", "キャンセル", "취소", "Отмена", "Annuler"],
  Region: ["更新地区", "更新地域", "업데이트 지역", "Регион обновлений", "Région des mises à jour"],
  Done: ["已完成", "完了", "완료", "Готово", "Terminé"],
  Waiting: ["等待中", "待機中", "대기 중", "Ожидание", "En attente"],
  Active: ["下载中", "ダウンロード中", "다운로드 중", "Загрузка", "En cours"],
  Applied: [
    "已设置桌面壁纸",
    "壁紙を設定しました",
    "배경화면 설정 완료",
    "Обои установлены",
    "Fond d’écran appliqué",
  ],
  Disconnected: ["显示器已断开", "画面が未接続", "연결이 끊긴 화면", "Экран отключён", "Écran déconnecté"],
  Empty: [
    "暂无下载任务",
    "ダウンロードはありません",
    "다운로드 없음",
    "Нет загрузок",
    "Aucun téléchargement",
  ],
  Wallpapers: ["张壁纸", "枚の壁紙", "개 배경화면", "обоев", "fonds d’écran"],
  Select: ["选择", "選択", "선택", "Выбрать", "Sélectionner"],
};
export function t(key: string) {
  return labels()[key] ?? key;
}
function accept(raw: string) {
  const value: Preferences = JSON.parse(raw);
  if (value.language !== preferences()?.language || !preferences()) {
    if (value.language === "system") setLocale(null);
    const lang = localeSettings().languages[0] ?? "en";
    const translated = JSON.parse(Library.labels(value.language, lang, JSON.stringify(keys)));
    const ix = ["zh", "ja", "ko", "ru", "fr"].indexOf(translated.locale.split("-")[0]);
    for (const [key, variants] of Object.entries(extras))
      translated.labels[key] = ix < 0 ? key : variants[ix];
    labels.set(translated.labels);
    setLocale(value.language === "system" ? null : translated.locale);
  }
  setColorScheme(value.theme_preference);
  preferences.set(value);
}
export async function loadPreferences() {
  accept(await Library.preferences());
}
export async function configure(patch: Partial<Preferences>, schedule = false) {
  const raw = JSON.stringify(patch);
  accept(await (schedule ? Library.configureSchedule(raw) : Library.configure(raw)));
}
export function run(action: () => Promise<unknown>) {
  void action().catch((error) => status.set(String(error)));
}
export async function startup(enabled: boolean) {
  const sys = (v: boolean) =>
    Library.hostOs() === "macos" ? Application.setLaunchAtLogin(v) : Library.configureStartup(v);
  await sys(enabled);
  try {
    await configure({ startup_enabled: enabled });
  } catch (error) {
    await sys(!enabled);
    throw error;
  }
}
