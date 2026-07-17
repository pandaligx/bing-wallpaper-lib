//! Application language selection and the small built-in translation catalog.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguagePreference {
    #[default]
    System,
    English,
    SimplifiedChinese,
    Japanese,
    Korean,
    Russian,
    French,
}

impl LanguagePreference {
    pub const ALL: [Self; 7] = [
        Self::System,
        Self::English,
        Self::SimplifiedChinese,
        Self::Japanese,
        Self::Korean,
        Self::Russian,
        Self::French,
    ];

    pub fn resolved(self) -> Self {
        if self != Self::System {
            return self;
        }

        let locale = system_locale().to_ascii_lowercase();
        if locale.starts_with("zh") {
            Self::SimplifiedChinese
        } else if locale.starts_with("ja") {
            Self::Japanese
        } else if locale.starts_with("ko") {
            Self::Korean
        } else if locale.starts_with("ru") {
            Self::Russian
        } else if locale.starts_with("fr") {
            Self::French
        } else {
            Self::English
        }
    }

    pub fn gpui_locale(self) -> &'static str {
        match self.resolved() {
            Self::SimplifiedChinese => "zh-CN",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Russian => "ru",
            Self::French => "fr",
            Self::English | Self::System => "en",
        }
    }

    pub fn flag(self) -> &'static str {
        match self {
            Self::System => "🌐",
            Self::English => "🇺🇸",
            Self::SimplifiedChinese => "🇨🇳",
            Self::Japanese => "🇯🇵",
            Self::Korean => "🇰🇷",
            Self::Russian => "🇷🇺",
            Self::French => "🇫🇷",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::English => "English",
            Self::SimplifiedChinese => "简体中文",
            Self::Japanese => "日本語",
            Self::Korean => "한국어",
            Self::Russian => "Русский",
            Self::French => "Français",
        }
    }

    pub fn t(self, key: &'static str) -> &'static str {
        translate(self.resolved(), key)
    }

    pub fn year_label(self, year: i32) -> String {
        match self.resolved() {
            Self::SimplifiedChinese | Self::Japanese => format!("{year} 年"),
            Self::Korean => format!("{year}년"),
            Self::Russian => format!("{year} г."),
            Self::English | Self::French | Self::System => year.to_string(),
        }
    }

    pub fn month_count_label(self, month: u32, count: usize) -> String {
        match self.resolved() {
            Self::SimplifiedChinese => format!("{month:02} 月 ({count} 张)"),
            Self::Japanese => format!("{month:02} 月 ({count} 枚)"),
            Self::Korean => format!("{month:02}월 ({count}장)"),
            Self::Russian => format!("{month:02} ({count})"),
            Self::French => format!("Mois {month:02} ({count})"),
            Self::English | Self::System => format!("Month {month:02} ({count})"),
        }
    }

    pub fn year_month_label(self, year: i32, month: u32) -> String {
        match self.resolved() {
            Self::SimplifiedChinese | Self::Japanese => format!("{year}年{month:02}月"),
            Self::Korean => format!("{year}년 {month:02}월"),
            Self::Russian => format!("{month:02}.{year}"),
            Self::French => format!("{month:02}/{year}"),
            Self::English | Self::System => format!("{year}-{month:02}"),
        }
    }
}

#[cfg(target_os = "windows")]
fn system_locale() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    // Windows documents LOCALE_NAME_MAX_LENGTH as 85 UTF-16 code units.
    let mut buffer = [0u16; 85];
    let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if length > 1 {
        String::from_utf16_lossy(&buffer[..length as usize - 1])
    } else {
        String::new()
    }
}

#[cfg(not(target_os = "windows"))]
fn system_locale() -> String {
    std::env::var("LANG").unwrap_or_default()
}

fn translate(language: LanguagePreference, key: &'static str) -> &'static str {
    match language {
        LanguagePreference::English => key,
        LanguagePreference::SimplifiedChinese => match key {
            "Home" => "主页",
            "Global resolution" => "全局分辨率",
            "Original" => "原图",
            "Default" => "默认",
            "Favorites" => "我的收藏",
            "Batch download" => "批量下载",
            "Downloaded wallpapers" => "已下载的壁纸",
            "Download center" => "下载中心",
            "Navigation" => "导航",
            "Archive" => "归档",
            "Settings" => "设置",
            "Language" => "语言",
            "Follow system" => "跟随系统",
            "Wallpaper library" => "每日 Bing 壁纸图库",
            "Loading wallpaper list..." => "正在加载壁纸列表...",
            "Image loading..." => "图片加载中...",
            "Download path" => "下载路径",
            "Appearance" => "外观模式",
            "Multi-monitor wallpaper" => "多显示器壁纸",
            "Automatic wallpaper" => "自动壁纸",
            "Maintenance" => "维护",
            "Close settings" => "关闭设置",
            "Wallpaper download folder" => "壁纸下载保存路径",
            "Open folder" => "打开目录",
            "Choose and save" => "选择并保存",
            "System" => "跟随系统",
            "Light" => "白天模式",
            "Dark" => "夜间模式",
            "Sync all displays" => "同步全部显示器",
            "Refresh" => "刷新",
            "Startup" => "开机自启",
            "Run in background / show tray icon" => "后台常驻 / 显示系统托盘图标",
            "Change wallpaper daily" => "每日自动更换壁纸",
            "Exit after automatic wallpaper" => "自动壁纸完成后退出程序",
            "Wallpaper source" => "壁纸来源",
            "Latest daily wallpaper" => "每日最新壁纸",
            "Random from all history" => "随机全部历史",
            "Random from favorites" => "随机我的收藏",
            "Execution time" => "执行时间",
            "Hour" => "小时",
            "Minute" => "分钟",
            "Current selection" => "当前选择",
            "Change once now" => "立即按当前方案更换一次",
            "Clear wallpaper cache" => "清空壁纸缓存",
            "Check for updates" => "检查更新",
            "About" => "关于软件",
            "Back to top" => "回到顶部",
            "Download" => "下载",
            "Set as wallpaper" => "设为桌面壁纸",
            "Preview" => "预览图片",
            "Cancel favorite" => "取消收藏",
            "Favorite" => "收藏",
            "No favorite wallpapers yet" => "还没有收藏壁纸",
            "No downloaded wallpapers yet" => "还没有已下载的壁纸",
            "Delete" => "删除",
            "Delete selected" => "删除选中项",
            "Select all" => "全选",
            "Clear selection" => "清除选择",
            "Refresh wallpaper list" => "重新获取壁纸列表",
            "Update available" => "发现新版本",
            "View release notes" => "查看更新内容",
            "Later" => "稍后再说",
            "Update now" => "立即更新",
            "Open main window" => "打开主窗口",
            "Run in background" => "后台常驻",
            "Exit" => "退出",
            "About subtitle" => "自动获取、浏览、下载并设置 Bing 每日壁纸",
            "Version" => "当前版本",
            "Copyright" => "版权信息",
            "Open source and credits" => "开源与致谢",
            "About data sources" => "近期壁纸来自 Bing 官方接口；历史归档来自 zxyongyo/bing-daily-wallpaper，并内置离线快照作为兜底。",
            "Historical archive" => "历史归档项目",
            "Project home" => "项目主页",
            "Download folder hint" => "留空则使用默认目录；保存后若路径不存在会自动创建。",
            "Theme hint" => "手动选择后不会再被系统主题变化覆盖。",
            "No display detected" => "未检测到可单独设置的显示器；将使用同步全部显示器。",
            "Display target hint" => "选择后，所有“设为桌面壁纸”操作都会按此目标生效。",
            "Auto exit hint" => "仅对每日自动执行生效；手动“立即更换一次”不会自动退出。",
            "Time selection hint" => "像闹钟一样分别选择小时和分钟。",
            "Automatic wallpaper hint" => "每日最新会下载当天图片；使用随机收藏前请先添加收藏。",
            "Recent wallpapers" => "最近壁纸",
            "Favorites empty hint" => "在首页或归档中点击 ❤ 即可收藏喜欢的壁纸。",
            "Quick download" => "快速下载",
            "All history" => "全部历史",
            "Current month" => "当前月份",
            "Download by date range" => "按日期范围下载",
            "Select a date range" => "请选择日期范围",
            "Download date range" => "下载日期范围",
            "Select a month from the sidebar" => "请在左侧选择月份",
            _ => key,
        },
        LanguagePreference::Japanese => match key {
            "Home" => "ホーム",
            "Global resolution" => "全体の解像度",
            "Favorites" => "お気に入り",
            "Batch download" => "一括ダウンロード",
            "Downloaded wallpapers" => "ダウンロード済み",
            "Download center" => "ダウンロードセンター",
            "Navigation" => "ナビゲーション",
            "Archive" => "アーカイブ",
            "Settings" => "設定",
            "Language" => "言語",
            "Follow system" => "システムに従う",
            "Wallpaper library" => "Bing デイリー壁紙ライブラリ",
            "Loading wallpaper list..." => "壁紙リストを読み込んでいます...",
            "Image loading..." => "画像を読み込んでいます...",
            "Download path" => "保存先",
            "Appearance" => "外観",
            "Multi-monitor wallpaper" => "マルチモニター",
            "Automatic wallpaper" => "自動壁紙",
            "Maintenance" => "メンテナンス",
            "Close settings" => "設定を閉じる",
            "Wallpaper download folder" => "壁紙の保存先",
            "Open folder" => "フォルダーを開く",
            "Choose and save" => "選択して保存",
            "System" => "システム",
            "Light" => "ライト",
            "Dark" => "ダーク",
            "Sync all displays" => "すべてのディスプレイに同期",
            "Refresh" => "更新",
            "Startup" => "自動起動",
            "Run in background / show tray icon" => "バックグラウンド実行 / トレイアイコン",
            "Change wallpaper daily" => "毎日壁紙を変更",
            "Exit after automatic wallpaper" => "自動変更後に終了",
            "Wallpaper source" => "壁紙の取得元",
            "Latest daily wallpaper" => "最新の壁紙",
            "Random from all history" => "履歴からランダム",
            "Random from favorites" => "お気に入りからランダム",
            "Execution time" => "実行時刻",
            "Hour" => "時",
            "Minute" => "分",
            "Current selection" => "現在の選択",
            "Change once now" => "今すぐ一度変更",
            "Clear wallpaper cache" => "壁紙キャッシュを消去",
            "Check for updates" => "更新を確認",
            "About" => "このアプリについて",
            "Back to top" => "トップへ",
            "Download" => "ダウンロード",
            "Set as wallpaper" => "壁紙に設定",
            "Preview" => "プレビュー",
            "Cancel favorite" => "お気に入り解除",
            "Favorite" => "お気に入り",
            "No favorite wallpapers yet" => "お気に入りはまだありません",
            "No downloaded wallpapers yet" => "ダウンロード済みの壁紙はありません",
            "Delete" => "削除",
            "Delete selected" => "選択項目を削除",
            "Select all" => "すべて選択",
            "Clear selection" => "選択解除",
            _ => key,
        },
        LanguagePreference::Korean => match key {
            "Home" => "홈",
            "Global resolution" => "전체 해상도",
            "Favorites" => "즐겨찾기",
            "Batch download" => "일괄 다운로드",
            "Downloaded wallpapers" => "다운로드한 배경화면",
            "Download center" => "다운로드 센터",
            "Navigation" => "탐색",
            "Archive" => "보관함",
            "Settings" => "설정",
            "Language" => "언어",
            "Follow system" => "시스템 설정 따르기",
            "Wallpaper library" => "Bing 데일리 배경화면 라이브러리",
            "Loading wallpaper list..." => "배경화면 목록을 불러오는 중...",
            "Image loading..." => "이미지 불러오는 중...",
            "Download path" => "다운로드 경로",
            "Appearance" => "화면 모드",
            "Multi-monitor wallpaper" => "다중 모니터",
            "Automatic wallpaper" => "자동 배경화면",
            "Maintenance" => "유지 관리",
            "Close settings" => "설정 닫기",
            "Wallpaper download folder" => "배경화면 저장 폴더",
            "Open folder" => "폴더 열기",
            "Choose and save" => "선택 후 저장",
            "System" => "시스템",
            "Light" => "라이트",
            "Dark" => "다크",
            "Sync all displays" => "모든 디스플레이 동기화",
            "Refresh" => "새로 고침",
            "Startup" => "시작 시 실행",
            "Run in background / show tray icon" => "백그라운드 실행 / 트레이 아이콘",
            "Change wallpaper daily" => "매일 배경화면 변경",
            "Exit after automatic wallpaper" => "자동 변경 후 종료",
            "Wallpaper source" => "배경화면 소스",
            "Latest daily wallpaper" => "최신 배경화면",
            "Random from all history" => "전체 기록에서 무작위",
            "Random from favorites" => "즐겨찾기에서 무작위",
            "Execution time" => "실행 시간",
            "Hour" => "시",
            "Minute" => "분",
            "Current selection" => "현재 선택",
            "Change once now" => "지금 한 번 변경",
            "Clear wallpaper cache" => "캐시 지우기",
            "Check for updates" => "업데이트 확인",
            "About" => "정보",
            "Back to top" => "맨 위로",
            "Download" => "다운로드",
            "Set as wallpaper" => "배경화면으로 설정",
            "Preview" => "미리보기",
            "Cancel favorite" => "즐겨찾기 해제",
            "Favorite" => "즐겨찾기",
            "No favorite wallpapers yet" => "즐겨찾기가 없습니다",
            "No downloaded wallpapers yet" => "다운로드한 배경화면이 없습니다",
            "Delete" => "삭제",
            "Delete selected" => "선택 항목 삭제",
            "Select all" => "전체 선택",
            "Clear selection" => "선택 해제",
            _ => key,
        },
        LanguagePreference::Russian => match key {
            "Home" => "Главная",
            "Global resolution" => "Общее разрешение",
            "Favorites" => "Избранное",
            "Batch download" => "Пакетная загрузка",
            "Downloaded wallpapers" => "Загруженные обои",
            "Download center" => "Центр загрузок",
            "Navigation" => "Навигация",
            "Archive" => "Архив",
            "Settings" => "Настройки",
            "Language" => "Язык",
            "Follow system" => "Как в системе",
            "Wallpaper library" => "Библиотека обоев Bing",
            "Loading wallpaper list..." => "Загрузка списка обоев...",
            "Image loading..." => "Загрузка изображения...",
            "Download path" => "Папка загрузки",
            "Appearance" => "Оформление",
            "Multi-monitor wallpaper" => "Несколько мониторов",
            "Automatic wallpaper" => "Автосмена обоев",
            "Maintenance" => "Обслуживание",
            "Close settings" => "Закрыть настройки",
            "Wallpaper download folder" => "Папка для обоев",
            "Open folder" => "Открыть папку",
            "Choose and save" => "Выбрать и сохранить",
            "System" => "Система",
            "Light" => "Светлая",
            "Dark" => "Тёмная",
            "Sync all displays" => "На все мониторы",
            "Refresh" => "Обновить",
            "Startup" => "Автозапуск",
            "Run in background / show tray icon" => "Фоновый режим / значок в трее",
            "Change wallpaper daily" => "Менять обои ежедневно",
            "Exit after automatic wallpaper" => "Выйти после автосмены",
            "Wallpaper source" => "Источник",
            "Latest daily wallpaper" => "Последние обои дня",
            "Random from all history" => "Случайные из истории",
            "Random from favorites" => "Случайные из избранного",
            "Execution time" => "Время запуска",
            "Hour" => "Час",
            "Minute" => "Минута",
            "Current selection" => "Выбрано",
            "Change once now" => "Сменить сейчас",
            "Clear wallpaper cache" => "Очистить кэш",
            "Check for updates" => "Проверить обновления",
            "About" => "О программе",
            "Back to top" => "Наверх",
            "Download" => "Скачать",
            "Set as wallpaper" => "Установить как обои",
            "Preview" => "Просмотр",
            "Cancel favorite" => "Убрать из избранного",
            "Favorite" => "В избранное",
            "No favorite wallpapers yet" => "В избранном пока пусто",
            "No downloaded wallpapers yet" => "Загруженных обоев пока нет",
            "Delete" => "Удалить",
            "Delete selected" => "Удалить выбранное",
            "Select all" => "Выбрать всё",
            "Clear selection" => "Снять выделение",
            _ => key,
        },
        LanguagePreference::French => match key {
            "Home" => "Accueil",
            "Global resolution" => "Résolution globale",
            "Favorites" => "Favoris",
            "Batch download" => "Téléchargement groupé",
            "Downloaded wallpapers" => "Fonds d’écran téléchargés",
            "Download center" => "Centre de téléchargement",
            "Navigation" => "Navigation",
            "Archive" => "Archives",
            "Settings" => "Paramètres",
            "Language" => "Langue",
            "Follow system" => "Suivre le système",
            "Wallpaper library" => "Bibliothèque quotidienne Bing",
            "Loading wallpaper list..." => "Chargement de la liste...",
            "Image loading..." => "Chargement de l’image...",
            "Download path" => "Dossier de téléchargement",
            "Appearance" => "Apparence",
            "Multi-monitor wallpaper" => "Multi-écrans",
            "Automatic wallpaper" => "Fond d’écran automatique",
            "Maintenance" => "Maintenance",
            "Close settings" => "Fermer les paramètres",
            "Wallpaper download folder" => "Dossier des fonds d’écran",
            "Open folder" => "Ouvrir le dossier",
            "Choose and save" => "Choisir et enregistrer",
            "System" => "Système",
            "Light" => "Clair",
            "Dark" => "Sombre",
            "Sync all displays" => "Synchroniser tous les écrans",
            "Refresh" => "Actualiser",
            "Startup" => "Lancer au démarrage",
            "Run in background / show tray icon" => "Arrière-plan / icône de notification",
            "Change wallpaper daily" => "Changer chaque jour",
            "Exit after automatic wallpaper" => "Quitter après le changement",
            "Wallpaper source" => "Source",
            "Latest daily wallpaper" => "Dernier fond du jour",
            "Random from all history" => "Aléatoire dans l’historique",
            "Random from favorites" => "Aléatoire dans les favoris",
            "Execution time" => "Heure d’exécution",
            "Hour" => "Heure",
            "Minute" => "Minute",
            "Current selection" => "Sélection actuelle",
            "Change once now" => "Changer maintenant",
            "Clear wallpaper cache" => "Vider le cache",
            "Check for updates" => "Rechercher des mises à jour",
            "About" => "À propos",
            "Back to top" => "Retour en haut",
            "Download" => "Télécharger",
            "Set as wallpaper" => "Définir comme fond d’écran",
            "Preview" => "Aperçu",
            "Cancel favorite" => "Retirer des favoris",
            "Favorite" => "Ajouter aux favoris",
            "No favorite wallpapers yet" => "Aucun favori pour le moment",
            "No downloaded wallpapers yet" => "Aucun fond téléchargé",
            "Delete" => "Supprimer",
            "Delete selected" => "Supprimer la sélection",
            "Select all" => "Tout sélectionner",
            "Clear selection" => "Effacer la sélection",
            _ => key,
        },
        LanguagePreference::System => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_language_has_a_locale_and_flag() {
        for language in LanguagePreference::ALL {
            assert!(!language.gpui_locale().is_empty());
            assert!(!language.flag().is_empty());
            assert!(!language.native_name().is_empty());
        }
    }

    #[test]
    fn core_navigation_is_translated() {
        assert_eq!(LanguagePreference::SimplifiedChinese.t("Home"), "主页");
        assert_eq!(LanguagePreference::Japanese.t("Settings"), "設定");
        assert_eq!(LanguagePreference::Korean.t("Language"), "언어");
        assert_eq!(LanguagePreference::Russian.t("Archive"), "Архив");
        assert_eq!(LanguagePreference::French.t("Favorites"), "Favoris");
    }
}
