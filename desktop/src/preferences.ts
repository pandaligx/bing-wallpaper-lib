import { localeSettings, setColorScheme, setTheme, signal } from "nsh";
import { setLocale } from "nsh/i18n";
import { Library } from "wallpaper-native";

export interface Preferences {
  language: string;
  smooth_wheel?: boolean;
  accent_color?: string | null;
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
export const smoothWheel = () => preferences()?.smooth_wheel ?? true;
export const status = signal("");
export const isError = signal(false);
export const retryNotice = signal<(() => void) | null>(null);
export function notify(message: string) {
  retryNotice.set(null);
  isError.set(false);
  status.set(message);
}
export function reportError(error: unknown, retry?: () => void) {
  retryNotice.set(retry ?? null);
  isError.set(true);
  const reason = error instanceof Error ? error.message : String(error);
  void Library.logError(reason).catch(() => {});
  status.set(reason.replace(/^(?:Error:\s*)+/, "").trim() || t("Failed"));
}
export const applied = signal(false);
export const canApply = signal(false);
const labels = signal<Record<string, string>>({});
const keys =
  "Home|Settings|Wallpaper library|Downloaded wallpapers|Favorites|Download center|Archive|Navigation|Appearance|Language|Follow system|Light|Dark|System|Default|Original|Wallpaper download folder|Choose and save|Open folder|Global resolution|Multi-monitor wallpaper|Sync all displays|No display detected|Automatic wallpaper|Startup|Run in background / show tray icon|Change wallpaper daily|Exit after automatic wallpaper|Wallpaper source|Latest daily wallpaper|Random from all history|Random from favorites|Execution time|Change once now|Enable periodic task scheduler|Use latest Bing wallpaper for the first run each day|Later wallpaper source|Repeat interval|Periodic interval range hint|Maintenance|Clear wallpaper cache|Check for updates|View release notes|About|Version|Copyright|Project home|Download|Set as wallpaper|Preview|Favorite|Cancel favorite|Refresh|Loading wallpaper list...|No favorite wallpapers yet|No downloaded wallpapers yet|Delete selected|Delete|Select all|Clear selection|Refresh wallpaper list|Back to top|Download by date range|Download date range|All history|Current month|Select a date range|Download progress|Failed|Skipped|Update now|Already up to date|Update available|Later|Open main window|Exit|Exit application|Minimize to tray|Exit app prompt".split(
    "|",
  );
keys.push("Speed", "Remaining");
const extras: Record<string, string[]> = {
  "Smooth mouse wheel": [
    "平滑滚轮",
    "ホイールを滑らかに",
    "부드러운 휠 스크롤",
    "Плавная прокрутка колеса",
    "Défilement fluide à la molette",
  ],
  "Local photos": ["本地图库", "ローカル写真", "로컬 사진", "Локальные фото", "Photos locales"],
  January: ["1月", "1月", "1월", "январь", "janvier"],
  February: ["2月", "2月", "2월", "февраль", "février"],
  March: ["3月", "3月", "3월", "март", "mars"],
  April: ["4月", "4月", "4월", "апрель", "avril"],
  May: ["5月", "5月", "5월", "май", "mai"],
  June: ["6月", "6月", "6월", "июнь", "juin"],
  July: ["7月", "7月", "7월", "июль", "juillet"],
  August: ["8月", "8月", "8월", "август", "août"],
  September: ["9月", "9月", "9월", "сентябрь", "septembre"],
  October: ["10月", "10月", "10월", "октябрь", "octobre"],
  November: ["11月", "11月", "11월", "ноябрь", "novembre"],
  December: ["12月", "12月", "12월", "декабрь", "décembre"],
  Theme: ["主题", "テーマ", "테마", "Тема", "Thème"],
  Storage: [
    "下载与存储",
    "ダウンロードと保存先",
    "다운로드 및 저장",
    "Загрузка и хранение",
    "Téléchargements et stockage",
  ],
  "Use HH:mm": [
    "请输入 00:00–23:59",
    "00:00–23:59 で入力",
    "00:00–23:59로 입력",
    "Введите время 00:00–23:59",
    "Saisissez une heure de 00:00 à 23:59",
  ],
  "Enter 1–1439 minutes": [
    "请输入 1–1439 分钟",
    "1–1439 分で入力",
    "1–1439분 입력",
    "Введите от 1 до 1439 минут",
    "Saisissez de 1 à 1439 minutes",
  ],
  Display: ["显示器", "ディスプレイ", "디스플레이", "Монитор", "Écran"],
  Close: ["关闭", "閉じる", "닫기", "Закрыть", "Fermer"],
  Previous: ["上一张", "前へ", "이전", "Назад", "Précédent"],
  Next: ["下一张", "次へ", "다음", "Далее", "Suivant"],
  "Bing Wallpaper Library": [
    "必应每日壁纸库",
    "Bing 毎日の壁紙",
    "Bing 일일 배경 화면",
    "Библиотека обоев Bing",
    "Photothèque Bing",
  ],
  Diagnostics: [
    "问题反馈与诊断",
    "フィードバックと診断",
    "피드백 및 진단",
    "Отзывы и диагностика",
    "Retours et diagnostic",
  ],
  "Copy log": ["复制日志", "ログをコピー", "로그 복사", "Копировать журнал", "Copier le journal"],
  "Open log": ["打开日志", "ログを開く", "로그 열기", "Открыть журнал", "Ouvrir le journal"],
  "Create issue": [
    "提交 GitHub 问题",
    "GitHub で報告",
    "GitHub에 문제 보고",
    "Сообщить в GitHub",
    "Signaler sur GitHub",
  ],
  Copied: ["已复制", "コピーしました", "복사됨", "Скопировано", "Copié"],
  Back: ["后退", "戻る", "뒤로", "Назад", "Retour"],
  Forward: ["前进", "進む", "앞으로", "Вперёд", "Avancer"],
  Blue: ["蓝色", "ブルー", "파랑", "Синий", "Bleu"],
  Purple: ["紫色", "パープル", "보라", "Фиолетовый", "Violet"],
  Green: ["绿色", "グリーン", "초록", "Зелёный", "Vert"],
  Zoom: ["缩放", "ズーム", "확대/축소", "Масштаб", "Zoom"],
  Resume: ["继续", "再開", "계속", "Продолжить", "Reprendre"],
  Cancelled: ["已取消", "キャンセル済み", "취소됨", "Отменено", "Annulé"],
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
const accents: Record<string, [string, string]> = {
  Blue: ["#0065cd", "#1378df"],
  Purple: ["#7045bc", "#8254c9"],
  Green: ["#157149", "#117849"],
};
let currentAccent = "";
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
  const accent = value.accent_color || "Blue";
  if (currentAccent !== accent) {
    const [light, dark] = accents[accent] ?? accents.Blue;
    setTheme({
      cssVars: {
        light: {
          background: "#f6f7f9",
          foreground: "#17191c",
          card: "#ffffff",
          sidebar: "#eef0f4",
          "sidebar-accent": "#dce2eb",
          primary: light,
          ring: light,
          "primary-foreground": "#ffffff",
        },
        dark: {
          background: "#151619",
          foreground: "#f2f3f5",
          card: "#222429",
          sidebar: "#1b1d21",
          "sidebar-accent": "#363b44",
          primary: dark,
          ring: dark,
          "primary-foreground": "#ffffff",
        },
      },
    });
    currentAccent = accent;
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
  void action().catch((error) => reportError(error));
}
export async function startup(enabled: boolean) {
  const sys = (v: boolean) => Library.configureStartup(v);
  await sys(enabled);
  try {
    await configure({ startup_enabled: enabled });
  } catch (error) {
    await sys(!enabled);
    throw error;
  }
}
