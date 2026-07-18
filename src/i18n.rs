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

    /// Localize runtime status text that may include dynamic values or backend errors.
    ///
    /// Older call sites build these messages in Chinese. Keeping the normalization here
    /// prevents a non-Chinese UI from mixing Chinese and English in the status banner.
    pub fn localize_status(self, message: &str) -> String {
        let language = self.resolved();
        if language == Self::SimplifiedChinese {
            return message.to_string();
        }

        if let Some(source) = message.strip_prefix("Automatic wallpaper source: ") {
            let prefix = match language {
                Self::Japanese => "自動壁紙の取得元",
                Self::Korean => "자동 배경화면 소스",
                Self::Russian => "Источник автоматических обоев",
                Self::French => "Source du fond automatique",
                Self::English | Self::System | Self::SimplifiedChinese => {
                    "Automatic wallpaper source"
                }
            };
            return format!("{prefix}: {source}");
        }

        if message.starts_with("Failed to save language setting:") {
            return runtime_status_text(language, RuntimeStatus::Failed).to_string();
        }

        if !message.chars().any(is_han) {
            return message.to_string();
        }

        let kind = if message.starts_with("共 ") && message.contains("张壁纸") {
            RuntimeStatus::Loaded
        } else if message.contains("正在重新获取壁纸列表") {
            RuntimeStatus::Refreshing
        } else if message.contains("已重新获取壁纸列表") {
            RuntimeStatus::Refreshed
        } else if message.contains("已加入收藏") {
            RuntimeStatus::FavoriteAdded
        } else if message.contains("已取消收藏") {
            RuntimeStatus::FavoriteRemoved
        } else if message.contains("已最小化到系统托盘") {
            RuntimeStatus::MinimizedToTray
        } else if message.contains("当前已是最新版本") {
            RuntimeStatus::AlreadyLatest
        } else if message.contains("正在下载新版本") {
            RuntimeStatus::DownloadingUpdate
        } else if message.contains("下载完成，即将重启") {
            RuntimeStatus::UpdateReady
        } else if message.starts_with("正在下载 ") || message.contains("壁纸下载中") {
            RuntimeStatus::Downloading
        } else if message.contains("已下载完成") {
            RuntimeStatus::DownloadComplete
        } else if message.starts_with("正在将 ") && message.contains("设置为") {
            RuntimeStatus::SettingWallpaper
        } else if message.starts_with("已将 ") && message.contains("设置为") {
            RuntimeStatus::WallpaperSet
        } else if message.starts_with("已开启") {
            RuntimeStatus::Enabled
        } else if message.starts_with("已关闭") {
            RuntimeStatus::Disabled
        } else if message.contains("失败") || message.contains("错误") || message.contains("异常")
        {
            RuntimeStatus::Failed
        } else if message.starts_with("请选择")
            || message.contains("不能")
            || message.contains("没有可")
            || message.contains("为空")
        {
            RuntimeStatus::ActionRequired
        } else if message.contains("正在") || message.contains("开始") {
            RuntimeStatus::Working
        } else {
            RuntimeStatus::Done
        };

        runtime_status_text(language, kind).to_string()
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

#[derive(Clone, Copy)]
enum RuntimeStatus {
    Loaded,
    Refreshing,
    Refreshed,
    FavoriteAdded,
    FavoriteRemoved,
    Enabled,
    Disabled,
    MinimizedToTray,
    Downloading,
    DownloadComplete,
    SettingWallpaper,
    WallpaperSet,
    AlreadyLatest,
    DownloadingUpdate,
    UpdateReady,
    ActionRequired,
    Failed,
    Working,
    Done,
}

fn runtime_status_text(language: LanguagePreference, status: RuntimeStatus) -> &'static str {
    match language {
        LanguagePreference::English | LanguagePreference::System => match status {
            RuntimeStatus::Loaded => "Wallpaper list loaded",
            RuntimeStatus::Refreshing => "Refreshing the wallpaper list...",
            RuntimeStatus::Refreshed => "Wallpaper list refreshed",
            RuntimeStatus::FavoriteAdded => "Added to favorites",
            RuntimeStatus::FavoriteRemoved => "Removed from favorites",
            RuntimeStatus::Enabled => "Setting enabled",
            RuntimeStatus::Disabled => "Setting disabled",
            RuntimeStatus::MinimizedToTray => "Minimized to the system tray; right-click the tray icon to exit",
            RuntimeStatus::Downloading => "Downloading wallpaper...",
            RuntimeStatus::DownloadComplete => "Download completed",
            RuntimeStatus::SettingWallpaper => "Setting the wallpaper...",
            RuntimeStatus::WallpaperSet => "Wallpaper set successfully",
            RuntimeStatus::AlreadyLatest => "You already have the latest version",
            RuntimeStatus::DownloadingUpdate => "Downloading the update...",
            RuntimeStatus::UpdateReady => "Download completed; restarting to finish the update...",
            RuntimeStatus::ActionRequired => "Please review the current selection",
            RuntimeStatus::Failed => "The operation failed",
            RuntimeStatus::Working => "Working...",
            RuntimeStatus::Done => "Operation completed",
        },
        LanguagePreference::Japanese => match status {
            RuntimeStatus::Loaded => "壁紙リストを読み込みました",
            RuntimeStatus::Refreshing => "壁紙リストを更新しています...",
            RuntimeStatus::Refreshed => "壁紙リストを更新しました",
            RuntimeStatus::FavoriteAdded => "お気に入りに追加しました",
            RuntimeStatus::FavoriteRemoved => "お気に入りから削除しました",
            RuntimeStatus::Enabled => "設定を有効にしました",
            RuntimeStatus::Disabled => "設定を無効にしました",
            RuntimeStatus::MinimizedToTray => "システムトレイに最小化しました。終了するにはトレイアイコンを右クリックしてください",
            RuntimeStatus::Downloading => "壁紙をダウンロードしています...",
            RuntimeStatus::DownloadComplete => "ダウンロードが完了しました",
            RuntimeStatus::SettingWallpaper => "壁紙を設定しています...",
            RuntimeStatus::WallpaperSet => "壁紙を設定しました",
            RuntimeStatus::AlreadyLatest => "最新バージョンです",
            RuntimeStatus::DownloadingUpdate => "更新をダウンロードしています...",
            RuntimeStatus::UpdateReady => "ダウンロードが完了しました。更新のため再起動します...",
            RuntimeStatus::ActionRequired => "現在の選択内容を確認してください",
            RuntimeStatus::Failed => "操作に失敗しました",
            RuntimeStatus::Working => "処理しています...",
            RuntimeStatus::Done => "操作が完了しました",
        },
        LanguagePreference::Korean => match status {
            RuntimeStatus::Loaded => "배경화면 목록을 불러왔습니다",
            RuntimeStatus::Refreshing => "배경화면 목록을 새로 고치는 중...",
            RuntimeStatus::Refreshed => "배경화면 목록을 새로 고쳤습니다",
            RuntimeStatus::FavoriteAdded => "즐겨찾기에 추가했습니다",
            RuntimeStatus::FavoriteRemoved => "즐겨찾기에서 제거했습니다",
            RuntimeStatus::Enabled => "설정을 켰습니다",
            RuntimeStatus::Disabled => "설정을 껐습니다",
            RuntimeStatus::MinimizedToTray => "시스템 트레이로 최소화했습니다. 종료하려면 트레이 아이콘을 오른쪽 클릭하세요",
            RuntimeStatus::Downloading => "배경화면을 다운로드하는 중...",
            RuntimeStatus::DownloadComplete => "다운로드를 완료했습니다",
            RuntimeStatus::SettingWallpaper => "배경화면을 설정하는 중...",
            RuntimeStatus::WallpaperSet => "배경화면을 설정했습니다",
            RuntimeStatus::AlreadyLatest => "현재 최신 버전입니다",
            RuntimeStatus::DownloadingUpdate => "업데이트를 다운로드하는 중...",
            RuntimeStatus::UpdateReady => "다운로드가 완료되어 업데이트를 위해 다시 시작합니다...",
            RuntimeStatus::ActionRequired => "현재 선택을 확인하세요",
            RuntimeStatus::Failed => "작업에 실패했습니다",
            RuntimeStatus::Working => "처리 중...",
            RuntimeStatus::Done => "작업을 완료했습니다",
        },
        LanguagePreference::Russian => match status {
            RuntimeStatus::Loaded => "Список обоев загружен",
            RuntimeStatus::Refreshing => "Обновление списка обоев...",
            RuntimeStatus::Refreshed => "Список обоев обновлён",
            RuntimeStatus::FavoriteAdded => "Добавлено в избранное",
            RuntimeStatus::FavoriteRemoved => "Удалено из избранного",
            RuntimeStatus::Enabled => "Настройка включена",
            RuntimeStatus::Disabled => "Настройка выключена",
            RuntimeStatus::MinimizedToTray => "Окно свёрнуто в системный трей; для выхода щёлкните значок правой кнопкой",
            RuntimeStatus::Downloading => "Загрузка обоев...",
            RuntimeStatus::DownloadComplete => "Загрузка завершена",
            RuntimeStatus::SettingWallpaper => "Установка обоев...",
            RuntimeStatus::WallpaperSet => "Обои успешно установлены",
            RuntimeStatus::AlreadyLatest => "Установлена последняя версия",
            RuntimeStatus::DownloadingUpdate => "Загрузка обновления...",
            RuntimeStatus::UpdateReady => "Загрузка завершена; перезапуск для установки обновления...",
            RuntimeStatus::ActionRequired => "Проверьте текущий выбор",
            RuntimeStatus::Failed => "Не удалось выполнить операцию",
            RuntimeStatus::Working => "Выполнение...",
            RuntimeStatus::Done => "Операция завершена",
        },
        LanguagePreference::French => match status {
            RuntimeStatus::Loaded => "Liste des fonds d’écran chargée",
            RuntimeStatus::Refreshing => "Actualisation de la liste...",
            RuntimeStatus::Refreshed => "Liste des fonds d’écran actualisée",
            RuntimeStatus::FavoriteAdded => "Ajouté aux favoris",
            RuntimeStatus::FavoriteRemoved => "Retiré des favoris",
            RuntimeStatus::Enabled => "Paramètre activé",
            RuntimeStatus::Disabled => "Paramètre désactivé",
            RuntimeStatus::MinimizedToTray => "Fenêtre réduite dans la zone de notification ; faites un clic droit sur l’icône pour quitter",
            RuntimeStatus::Downloading => "Téléchargement du fond d’écran...",
            RuntimeStatus::DownloadComplete => "Téléchargement terminé",
            RuntimeStatus::SettingWallpaper => "Définition du fond d’écran...",
            RuntimeStatus::WallpaperSet => "Fond d’écran défini",
            RuntimeStatus::AlreadyLatest => "Vous utilisez déjà la dernière version",
            RuntimeStatus::DownloadingUpdate => "Téléchargement de la mise à jour...",
            RuntimeStatus::UpdateReady => "Téléchargement terminé ; redémarrage pour finaliser la mise à jour...",
            RuntimeStatus::ActionRequired => "Vérifiez la sélection actuelle",
            RuntimeStatus::Failed => "L’opération a échoué",
            RuntimeStatus::Working => "Traitement en cours...",
            RuntimeStatus::Done => "Opération terminée",
        },
        LanguagePreference::SimplifiedChinese => match status {
            RuntimeStatus::Loaded => "壁纸列表已加载",
            RuntimeStatus::Refreshing => "正在刷新壁纸列表...",
            RuntimeStatus::Refreshed => "壁纸列表已刷新",
            RuntimeStatus::FavoriteAdded => "已加入收藏",
            RuntimeStatus::FavoriteRemoved => "已取消收藏",
            RuntimeStatus::Enabled => "设置已开启",
            RuntimeStatus::Disabled => "设置已关闭",
            RuntimeStatus::MinimizedToTray => "已最小化到系统托盘",
            RuntimeStatus::Downloading => "正在下载壁纸...",
            RuntimeStatus::DownloadComplete => "下载完成",
            RuntimeStatus::SettingWallpaper => "正在设置壁纸...",
            RuntimeStatus::WallpaperSet => "壁纸设置完成",
            RuntimeStatus::AlreadyLatest => "当前已是最新版本",
            RuntimeStatus::DownloadingUpdate => "正在下载更新...",
            RuntimeStatus::UpdateReady => "下载完成，即将重启更新...",
            RuntimeStatus::ActionRequired => "请检查当前选择",
            RuntimeStatus::Failed => "操作失败",
            RuntimeStatus::Working => "正在处理...",
            RuntimeStatus::Done => "操作完成",
        },
    }
}

fn is_han(ch: char) -> bool {
    matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}')
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
            "Windows scheduled wallpaper" => "Windows 计划任务壁纸",
            "Enable periodic task scheduler" => "启用登录及周期自动换壁纸",
            "Use latest Bing wallpaper for the first run each day" => "每天首次执行时使用 Bing 最新壁纸",
            "Later wallpaper source" => "当天后续壁纸来源",
            "Repeat interval" => "重复间隔",
            "Periodic interval range hint" => "范围为 1 分钟至 23 小时 59 分钟；00:00 会自动调整为 00:01。",
            "Periodic task hint" => "启用后会在当前用户登录时执行，并按所选间隔重复；错过的任务会在电脑恢复可用后补执行一次，但不会唤醒电脑。每次换完壁纸即退出；收藏为空时自动回退到随机历史壁纸。启用此功能会关闭旧的开机自启。",
            "Startup disabled by periodic task hint" => "启用计划任务后，旧的开机自启会被关闭且不可同时开启。",
            "Disable periodic task before enabling startup" => "请先关闭周期计划任务，再开启旧的开机自启。",
            "Periodic task enabled" => "已启用 Windows 周期壁纸任务",
            "Periodic task disabled" => "已关闭 Windows 周期壁纸任务",
            "Periodic interval updated" => "周期任务间隔已更新",
            "Periodic settings saved" => "周期壁纸设置已保存",
            "Periodic wallpaper source" => "周期壁纸来源",
            "Failed to enable periodic task" => "启用 Windows 周期壁纸任务失败",
            "Failed to disable periodic task" => "关闭 Windows 周期壁纸任务失败",
            "Failed to update periodic task" => "更新 Windows 周期壁纸任务失败",
            "Failed to save periodic settings" => "保存周期壁纸设置失败",
            "Failed to disable old startup" => "关闭旧的开机自启失败",
            "Recent wallpapers" => "最近壁纸",
            "Favorites empty hint" => "在首页或归档中点击 ❤ 即可收藏喜欢的壁纸。",
            "Quick download" => "快速下载",
            "All history" => "全部历史",
            "Current month" => "当前月份",
            "Download by date range" => "按日期范围下载",
            "Select a date range" => "请选择日期范围",
            "Download date range" => "下载日期范围",
            "Select a month from the sidebar" => "请在左侧选择月份",
            "Exit app title" => "退出必应每日壁纸库？",
            "Exit app prompt" => "后台常驻当前未开启。你想直接退出程序，还是仅最小化到系统托盘继续后台运行？",
            "Minimize tray hint" => "选择“最小化到托盘”不会自动开启开机自启；如需开机后台运行，请在设置里开启开机自启。",
            "Minimize to tray" => "最小化到托盘",
            "Exit application" => "退出程序",
            "Download wallpaper tooltip" => "下载当前高清壁纸到本地目录",
            "Set wallpaper tooltip" => "自动下载并按当前显示器设置应用为桌面壁纸",
            "Refresh list tooltip" => "重新从远程数据源获取壁纸列表；网络不可用时会继续使用内置列表",
            "Already up to date" => "当前已是最新版本",
            "Update check failed" => "检查更新失败",
            "New version prompt" => "发现新版本 v{version}（当前 v{current}），是否立即下载并更新？",
            "Downloading version" => "正在下载新版本 v{version}",
            "Speed" => "速度",
            "Remaining" => "剩余",
            "Update restart hint" => "下载完成后应用会自动重启完成更新，请勿关闭。",
            "Wallpaper downloading..." => "壁纸下载中...",
            "Thumbnail generation failed" => "缩略图生成失败，请点击刷新重试",
            "Cannot read local image" => "无法读取本地图片",
            "Generating thumbnail..." => "正在生成缩略图...",
            "Available date range" => "可选范围：{start} 至 {end}；超出范围的日期会自动禁用。",
            "Date range unavailable" => "壁纸列表加载完成后才能选择日期范围。",
            "Download all history tooltip" => "下载当前列表中的全部历史壁纸",
            "Download current month tooltip" => "下载左侧当前选中月份的壁纸",
            "Download favorites tooltip" => "下载我的收藏中的全部壁纸",
            "Download range tooltip" => "下载日历中选中的日期范围壁纸",
            "Download progress" => "下载进度",
            "Skipped" => "跳过",
            "Failed" => "失败",
            "Select all downloaded tooltip" => "全选或取消全选当前下载目录中的壁纸",
            "Delete selected tooltip" => "删除当前勾选的本地壁纸文件",
            "Rescan downloads tooltip" => "重新扫描下载目录",
            "Open downloads tooltip" => "在资源管理器中打开当前壁纸下载目录",
            "Downloaded empty hint" => "在首页、归档或批量下载中下载壁纸后会显示在这里",
            "Select local wallpaper tooltip" => "选择这张本地壁纸用于批量删除",
            "Set local wallpaper tooltip" => "将这张本地图片设置为桌面壁纸",
            "Delete local wallpaper tooltip" => "删除这张本地壁纸文件",
            "Preview image tooltip" => "打开高清大图预览",
            _ => key,
        },
        LanguagePreference::Japanese => match key {
            "Home" => "ホーム",
            "Global resolution" => "全体の解像度",
            "Original" => "オリジナル",
            "Default" => "既定",
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
            "Refresh wallpaper list" => "壁紙リストを更新",
            "Update available" => "新しいバージョンがあります",
            "View release notes" => "リリースノートを表示",
            "Later" => "後で",
            "Update now" => "今すぐ更新",
            "Open main window" => "メインウィンドウを開く",
            "Run in background" => "バックグラウンド実行",
            "Exit" => "終了",
            "About subtitle" => "Bing のデイリー壁紙を取得・閲覧・ダウンロード・設定",
            "Version" => "バージョン",
            "Copyright" => "著作権情報",
            "Open source and credits" => "オープンソースとクレジット",
            "About data sources" => "最近の壁紙は Bing 公式 API、履歴は zxyongyo/bing-daily-wallpaper から取得し、オフラインスナップショットも内蔵しています。",
            "Historical archive" => "履歴アーカイブ",
            "Project home" => "プロジェクトホーム",
            "Download folder hint" => "空欄の場合は既定のフォルダーを使用し、存在しない場合は保存時に作成します。",
            "Theme hint" => "手動で選択すると、システムテーマの変更では上書きされません。",
            "No display detected" => "個別に設定できるディスプレイが見つかりません。すべてのディスプレイに同期します。",
            "Display target hint" => "以後のすべての「壁紙に設定」操作にこの対象が使用されます。",
            "Auto exit hint" => "毎日の自動実行にのみ適用されます。手動実行では終了しません。",
            "Time selection hint" => "時と分を個別に選択してください。",
            "Automatic wallpaper hint" => "最新を選ぶと当日の画像を取得します。お気に入りから選ぶ前に壁紙を登録してください。",
            "Windows scheduled wallpaper" => "Windows タスク スケジューラ壁紙",
            "Enable periodic task scheduler" => "ログオン時と一定間隔で壁紙を変更する",
            "Use latest Bing wallpaper for the first run each day" => "毎日の初回は最新の Bing 壁紙を使用する",
            "Later wallpaper source" => "同日の2回目以降の壁紙ソース",
            "Repeat interval" => "繰り返し間隔",
            "Periodic interval range hint" => "1分～23時間59分。00:00 は自動的に 00:01 になります。",
            "Periodic task hint" => "現在のユーザーのログオン時と指定間隔で実行します。実行できなかった場合は、PCが利用可能になった時に1回実行しますが、スリープ解除はしません。壁紙変更後は毎回終了し、お気に入りが空なら履歴からランダムに選びます。有効にすると従来の自動起動は無効になります。",
            "Startup disabled by periodic task hint" => "定期タスクの有効中は従来の自動起動を同時に使用できません。",
            "Disable periodic task before enabling startup" => "先に定期タスクを無効にしてから自動起動を有効にしてください。",
            "Periodic task enabled" => "Windows 定期壁紙タスクを有効にしました",
            "Periodic task disabled" => "Windows 定期壁紙タスクを無効にしました",
            "Periodic interval updated" => "定期タスクの間隔を更新しました",
            "Periodic settings saved" => "定期壁紙の設定を保存しました",
            "Periodic wallpaper source" => "定期壁紙のソース",
            "Failed to enable periodic task" => "Windows 定期壁紙タスクを有効にできませんでした",
            "Failed to disable periodic task" => "Windows 定期壁紙タスクを無効にできませんでした",
            "Failed to update periodic task" => "Windows 定期壁紙タスクを更新できませんでした",
            "Failed to save periodic settings" => "定期壁紙の設定を保存できませんでした",
            "Failed to disable old startup" => "従来の自動起動を無効にできませんでした",
            "Recent wallpapers" => "最近の壁紙",
            "Favorites empty hint" => "ホームまたはアーカイブで ❤ を押すと壁紙をお気に入りに追加できます。",
            "Quick download" => "クイックダウンロード",
            "All history" => "すべての履歴",
            "Current month" => "今月",
            "Download by date range" => "期間を指定してダウンロード",
            "Select a date range" => "期間を選択してください",
            "Download date range" => "選択期間をダウンロード",
            "Select a month from the sidebar" => "左側で月を選択してください",
            "Exit app title" => "Bing 壁紙ライブラリを終了しますか？",
            "Exit app prompt" => "バックグラウンド実行は現在無効です。アプリを終了しますか、それともシステムトレイに最小化しますか？",
            "Minimize tray hint" => "トレイへの最小化では自動起動は有効になりません。必要な場合は設定で自動起動を有効にしてください。",
            "Minimize to tray" => "トレイに最小化",
            "Exit application" => "アプリを終了",
            "Download wallpaper tooltip" => "現在の高解像度壁紙を保存します",
            "Set wallpaper tooltip" => "自動的にダウンロードし、現在のディスプレイ設定に適用します",
            "Refresh list tooltip" => "リモートデータから壁紙リストを更新します。接続できない場合は内蔵リストを使用します",
            "Already up to date" => "最新バージョンです",
            "Update check failed" => "更新の確認に失敗しました",
            "New version prompt" => "新しいバージョン v{version}（現在 v{current}）があります。今すぐ更新しますか？",
            "Downloading version" => "バージョン v{version} をダウンロード中",
            "Speed" => "速度",
            "Remaining" => "残り",
            "Update restart hint" => "ダウンロード後に自動的に再起動して更新します。アプリを閉じないでください。",
            "Wallpaper downloading..." => "壁紙をダウンロードしています...",
            "Thumbnail generation failed" => "サムネイルの作成に失敗しました。更新して再試行してください",
            "Cannot read local image" => "ローカル画像を読み込めません",
            "Generating thumbnail..." => "サムネイルを作成しています...",
            "Available date range" => "選択可能期間: {start} ～ {end}。範囲外の日付は無効です。",
            "Date range unavailable" => "壁紙リストの読み込み後に期間を選択できます。",
            "Download all history tooltip" => "履歴内のすべての壁紙をダウンロード",
            "Download current month tooltip" => "左側で選択した月の壁紙をダウンロード",
            "Download favorites tooltip" => "お気に入りの壁紙をすべてダウンロード",
            "Download range tooltip" => "カレンダーで選択した期間をダウンロード",
            "Download progress" => "ダウンロード状況",
            "Skipped" => "スキップ",
            "Failed" => "失敗",
            "Select all downloaded tooltip" => "現在のフォルダー内の壁紙をすべて選択または解除",
            "Delete selected tooltip" => "選択したローカル壁紙を削除",
            "Rescan downloads tooltip" => "ダウンロードフォルダーを再読み込み",
            "Open downloads tooltip" => "ダウンロードフォルダーをエクスプローラーで開く",
            "Downloaded empty hint" => "ホーム、アーカイブ、一括ダウンロードから保存した壁紙がここに表示されます",
            "Select local wallpaper tooltip" => "一括削除するローカル壁紙を選択",
            "Set local wallpaper tooltip" => "このローカル画像を壁紙に設定",
            "Delete local wallpaper tooltip" => "このローカル壁紙ファイルを削除",
            "Preview image tooltip" => "高解像度画像をプレビュー",
            _ => key,
        },
        LanguagePreference::Korean => match key {
            "Home" => "홈",
            "Global resolution" => "전체 해상도",
            "Original" => "원본",
            "Default" => "기본값",
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
            "Refresh wallpaper list" => "배경화면 목록 새로 고침",
            "Update available" => "새 버전 발견",
            "View release notes" => "업데이트 내용 보기",
            "Later" => "나중에",
            "Update now" => "지금 업데이트",
            "Open main window" => "메인 창 열기",
            "Run in background" => "백그라운드 실행",
            "Exit" => "종료",
            "About subtitle" => "Bing 데일리 배경화면을 자동으로 가져오고 탐색, 다운로드 및 설정합니다",
            "Version" => "버전",
            "Copyright" => "저작권 정보",
            "Open source and credits" => "오픈 소스 및 감사",
            "About data sources" => "최근 배경화면은 Bing 공식 API에서, 기록은 zxyongyo/bing-daily-wallpaper에서 가져오며 오프라인 스냅샷도 포함합니다.",
            "Historical archive" => "기록 보관소",
            "Project home" => "프로젝트 홈",
            "Download folder hint" => "비워 두면 기본 폴더를 사용하며, 폴더가 없으면 저장할 때 자동 생성합니다.",
            "Theme hint" => "수동으로 선택하면 시스템 테마 변경으로 덮어쓰지 않습니다.",
            "No display detected" => "개별 설정 가능한 디스플레이가 없습니다. 모든 디스플레이에 동기화합니다.",
            "Display target hint" => "이후 모든 ‘배경화면으로 설정’ 작업에 이 대상이 적용됩니다.",
            "Auto exit hint" => "매일 자동 실행에만 적용되며 수동 실행 후에는 종료하지 않습니다.",
            "Time selection hint" => "시와 분을 각각 선택하세요.",
            "Automatic wallpaper hint" => "최신 항목은 오늘 이미지를 받습니다. 즐겨찾기 무작위를 사용하기 전에 항목을 추가하세요.",
            "Windows scheduled wallpaper" => "Windows 작업 스케줄러 배경화면",
            "Enable periodic task scheduler" => "로그온 및 주기적 배경화면 변경 사용",
            "Use latest Bing wallpaper for the first run each day" => "매일 첫 실행에는 최신 Bing 배경화면 사용",
            "Later wallpaper source" => "같은 날 이후 배경화면 소스",
            "Repeat interval" => "반복 간격",
            "Periodic interval range hint" => "1분부터 23시간 59분까지입니다. 00:00은 자동으로 00:01로 조정됩니다.",
            "Periodic task hint" => "현재 사용자 로그온 시 및 선택한 간격마다 실행합니다. 놓친 작업은 PC를 다시 사용할 수 있을 때 한 번 실행하지만 절전 모드를 해제하지는 않습니다. 배경화면을 바꾼 뒤 매번 종료하며, 즐겨찾기가 비어 있으면 전체 기록에서 무작위로 선택합니다. 이 기능을 켜면 기존 시작 프로그램이 꺼집니다.",
            "Startup disabled by periodic task hint" => "주기 작업을 사용하는 동안 기존 시작 프로그램을 동시에 켤 수 없습니다.",
            "Disable periodic task before enabling startup" => "먼저 주기 작업을 끈 다음 기존 시작 프로그램을 켜세요.",
            "Periodic task enabled" => "Windows 주기 배경화면 작업을 켰습니다",
            "Periodic task disabled" => "Windows 주기 배경화면 작업을 껐습니다",
            "Periodic interval updated" => "주기 작업 간격을 업데이트했습니다",
            "Periodic settings saved" => "주기 배경화면 설정을 저장했습니다",
            "Periodic wallpaper source" => "주기 배경화면 소스",
            "Failed to enable periodic task" => "Windows 주기 배경화면 작업을 켜지 못했습니다",
            "Failed to disable periodic task" => "Windows 주기 배경화면 작업을 끄지 못했습니다",
            "Failed to update periodic task" => "Windows 주기 배경화면 작업을 업데이트하지 못했습니다",
            "Failed to save periodic settings" => "주기 배경화면 설정을 저장하지 못했습니다",
            "Failed to disable old startup" => "기존 시작 프로그램을 끄지 못했습니다",
            "Recent wallpapers" => "최근 배경화면",
            "Favorites empty hint" => "홈 또는 보관함에서 ❤를 눌러 배경화면을 즐겨찾기에 추가하세요.",
            "Quick download" => "빠른 다운로드",
            "All history" => "전체 기록",
            "Current month" => "현재 월",
            "Download by date range" => "날짜 범위로 다운로드",
            "Select a date range" => "날짜 범위를 선택하세요",
            "Download date range" => "선택한 날짜 범위 다운로드",
            "Select a month from the sidebar" => "왼쪽에서 월을 선택하세요",
            "Exit app title" => "Bing 배경화면 라이브러리를 종료할까요?",
            "Exit app prompt" => "백그라운드 실행이 꺼져 있습니다. 앱을 종료할까요, 아니면 시스템 트레이로 최소화할까요?",
            "Minimize tray hint" => "트레이로 최소화해도 시작 프로그램은 켜지지 않습니다. 필요하면 설정에서 시작 시 실행을 켜세요.",
            "Minimize to tray" => "트레이로 최소화",
            "Exit application" => "앱 종료",
            "Download wallpaper tooltip" => "현재 고해상도 배경화면을 로컬 폴더에 저장합니다",
            "Set wallpaper tooltip" => "자동으로 다운로드하고 현재 모니터 설정에 적용합니다",
            "Refresh list tooltip" => "원격 데이터에서 목록을 새로 고칩니다. 네트워크를 사용할 수 없으면 내장 목록을 유지합니다",
            "Already up to date" => "현재 최신 버전입니다",
            "Update check failed" => "업데이트 확인에 실패했습니다",
            "New version prompt" => "새 버전 v{version}(현재 v{current})을 사용할 수 있습니다. 지금 업데이트할까요?",
            "Downloading version" => "버전 v{version} 다운로드 중",
            "Speed" => "속도",
            "Remaining" => "남은 시간",
            "Update restart hint" => "다운로드가 끝나면 업데이트를 위해 자동으로 다시 시작합니다. 앱을 닫지 마세요.",
            "Wallpaper downloading..." => "배경화면 다운로드 중...",
            "Thumbnail generation failed" => "썸네일 생성에 실패했습니다. 새로 고쳐 다시 시도하세요",
            "Cannot read local image" => "로컬 이미지를 읽을 수 없습니다",
            "Generating thumbnail..." => "썸네일 생성 중...",
            "Available date range" => "선택 가능 범위: {start} ~ {end}. 범위 밖 날짜는 비활성화됩니다.",
            "Date range unavailable" => "배경화면 목록을 불러온 후 날짜 범위를 선택할 수 있습니다.",
            "Download all history tooltip" => "전체 기록의 배경화면 다운로드",
            "Download current month tooltip" => "왼쪽에서 선택한 월의 배경화면 다운로드",
            "Download favorites tooltip" => "즐겨찾기 배경화면 모두 다운로드",
            "Download range tooltip" => "달력에서 선택한 날짜 범위 다운로드",
            "Download progress" => "다운로드 진행률",
            "Skipped" => "건너뜀",
            "Failed" => "실패",
            "Select all downloaded tooltip" => "현재 다운로드 폴더의 배경화면 전체 선택 또는 해제",
            "Delete selected tooltip" => "선택한 로컬 배경화면 삭제",
            "Rescan downloads tooltip" => "다운로드 폴더 다시 검색",
            "Open downloads tooltip" => "파일 탐색기에서 다운로드 폴더 열기",
            "Downloaded empty hint" => "홈, 보관함 또는 일괄 다운로드에서 받은 배경화면이 여기에 표시됩니다",
            "Select local wallpaper tooltip" => "일괄 삭제할 로컬 배경화면 선택",
            "Set local wallpaper tooltip" => "이 로컬 이미지를 배경화면으로 설정",
            "Delete local wallpaper tooltip" => "이 로컬 배경화면 파일 삭제",
            "Preview image tooltip" => "고해상도 이미지 미리보기",
            _ => key,
        },
        LanguagePreference::Russian => match key {
            "Home" => "Главная",
            "Global resolution" => "Общее разрешение",
            "Original" => "Оригинал",
            "Default" => "По умолчанию",
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
            "Refresh wallpaper list" => "Обновить список обоев",
            "Update available" => "Доступна новая версия",
            "View release notes" => "Посмотреть изменения",
            "Later" => "Позже",
            "Update now" => "Обновить сейчас",
            "Open main window" => "Открыть главное окно",
            "Run in background" => "Работать в фоне",
            "Exit" => "Выход",
            "About subtitle" => "Получение, просмотр, загрузка и установка ежедневных обоев Bing",
            "Version" => "Версия",
            "Copyright" => "Авторские права",
            "Open source and credits" => "Открытый код и благодарности",
            "About data sources" => "Свежие обои загружаются через официальный API Bing, архив — из zxyongyo/bing-daily-wallpaper; также встроена автономная копия.",
            "Historical archive" => "Исторический архив",
            "Project home" => "Страница проекта",
            "Download folder hint" => "Если поле пустое, используется стандартная папка; отсутствующая папка будет создана при сохранении.",
            "Theme hint" => "Ручной выбор не будет заменён при изменении системной темы.",
            "No display detected" => "Отдельные мониторы не обнаружены; обои будут синхронизированы на всех экранах.",
            "Display target hint" => "Все последующие команды установки обоев будут применяться к этой цели.",
            "Auto exit hint" => "Только для ежедневного запуска; после ручной смены приложение не закрывается.",
            "Time selection hint" => "Выберите часы и минуты отдельно.",
            "Automatic wallpaper hint" => "Последние обои загружают изображение дня. Для случайного выбора сначала добавьте избранное.",
            "Windows scheduled wallpaper" => "Обои по расписанию Windows",
            "Enable periodic task scheduler" => "Менять обои при входе и по интервалу",
            "Use latest Bing wallpaper for the first run each day" => "При первом запуске за день ставить последние обои Bing",
            "Later wallpaper source" => "Источник для следующих запусков за день",
            "Repeat interval" => "Интервал повтора",
            "Periodic interval range hint" => "От 1 минуты до 23 часов 59 минут; 00:00 автоматически станет 00:01.",
            "Periodic task hint" => "Задача запускается при входе текущего пользователя и через выбранный интервал. Пропущенный запуск будет выполнен один раз, когда ПК снова станет доступен, без пробуждения из сна. После смены обоев программа завершится; если избранное пусто, будут выбраны случайные обои из истории. Включение отключает прежний автозапуск.",
            "Startup disabled by periodic task hint" => "При включённой периодической задаче прежний автозапуск недоступен.",
            "Disable periodic task before enabling startup" => "Сначала отключите периодическую задачу, затем включите прежний автозапуск.",
            "Periodic task enabled" => "Периодическая задача Windows включена",
            "Periodic task disabled" => "Периодическая задача Windows отключена",
            "Periodic interval updated" => "Интервал периодической задачи обновлён",
            "Periodic settings saved" => "Настройки периодической смены сохранены",
            "Periodic wallpaper source" => "Источник периодических обоев",
            "Failed to enable periodic task" => "Не удалось включить периодическую задачу Windows",
            "Failed to disable periodic task" => "Не удалось отключить периодическую задачу Windows",
            "Failed to update periodic task" => "Не удалось обновить периодическую задачу Windows",
            "Failed to save periodic settings" => "Не удалось сохранить настройки периодической смены",
            "Failed to disable old startup" => "Не удалось отключить прежний автозапуск",
            "Recent wallpapers" => "Недавние обои",
            "Favorites empty hint" => "Нажмите ❤ на главной странице или в архиве, чтобы добавить обои в избранное.",
            "Quick download" => "Быстрая загрузка",
            "All history" => "Вся история",
            "Current month" => "Текущий месяц",
            "Download by date range" => "Загрузка по диапазону дат",
            "Select a date range" => "Выберите диапазон дат",
            "Download date range" => "Скачать выбранный диапазон",
            "Select a month from the sidebar" => "Выберите месяц слева",
            "Exit app title" => "Выйти из библиотеки обоев Bing?",
            "Exit app prompt" => "Фоновый режим выключен. Выйти из приложения или свернуть его в системный трей?",
            "Minimize tray hint" => "Сворачивание в трей не включает автозапуск. При необходимости включите его в настройках.",
            "Minimize to tray" => "Свернуть в трей",
            "Exit application" => "Выйти из приложения",
            "Download wallpaper tooltip" => "Скачать текущие обои высокого разрешения",
            "Set wallpaper tooltip" => "Скачать и применить обои с текущими настройками мониторов",
            "Refresh list tooltip" => "Обновить список из удалённых источников; без сети будет использован встроенный список",
            "Already up to date" => "Установлена последняя версия",
            "Update check failed" => "Не удалось проверить обновления",
            "New version prompt" => "Доступна версия v{version} (сейчас v{current}). Обновить сейчас?",
            "Downloading version" => "Загрузка версии v{version}",
            "Speed" => "Скорость",
            "Remaining" => "Осталось",
            "Update restart hint" => "После загрузки приложение автоматически перезапустится для обновления. Не закрывайте его.",
            "Wallpaper downloading..." => "Загрузка обоев...",
            "Thumbnail generation failed" => "Не удалось создать миниатюру. Обновите список и повторите попытку",
            "Cannot read local image" => "Не удалось прочитать локальное изображение",
            "Generating thumbnail..." => "Создание миниатюры...",
            "Available date range" => "Доступный диапазон: {start} — {end}. Остальные даты отключены.",
            "Date range unavailable" => "Диапазон дат станет доступен после загрузки списка обоев.",
            "Download all history tooltip" => "Скачать все обои из истории",
            "Download current month tooltip" => "Скачать обои выбранного слева месяца",
            "Download favorites tooltip" => "Скачать все обои из избранного",
            "Download range tooltip" => "Скачать обои за выбранный в календаре период",
            "Download progress" => "Ход загрузки",
            "Skipped" => "Пропущено",
            "Failed" => "Ошибок",
            "Select all downloaded tooltip" => "Выбрать или снять выбор со всех обоев в папке",
            "Delete selected tooltip" => "Удалить выбранные локальные обои",
            "Rescan downloads tooltip" => "Повторно просканировать папку загрузок",
            "Open downloads tooltip" => "Открыть папку загрузок в Проводнике",
            "Downloaded empty hint" => "Скачанные с главной страницы, из архива или пакетной загрузки обои появятся здесь",
            "Select local wallpaper tooltip" => "Выбрать эти локальные обои для пакетного удаления",
            "Set local wallpaper tooltip" => "Установить локальное изображение как обои",
            "Delete local wallpaper tooltip" => "Удалить этот локальный файл обоев",
            "Preview image tooltip" => "Открыть изображение высокого разрешения",
            _ => key,
        },
        LanguagePreference::French => match key {
            "Home" => "Accueil",
            "Global resolution" => "Résolution globale",
            "Original" => "Original",
            "Default" => "Par défaut",
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
            "Refresh wallpaper list" => "Actualiser la liste",
            "Update available" => "Nouvelle version disponible",
            "View release notes" => "Voir les notes de version",
            "Later" => "Plus tard",
            "Update now" => "Mettre à jour maintenant",
            "Open main window" => "Ouvrir la fenêtre principale",
            "Run in background" => "Exécuter en arrière-plan",
            "Exit" => "Quitter",
            "About subtitle" => "Récupérer, parcourir, télécharger et définir les fonds d’écran quotidiens Bing",
            "Version" => "Version",
            "Copyright" => "Droits d’auteur",
            "Open source and credits" => "Logiciels libres et crédits",
            "About data sources" => "Les fonds récents viennent de l’API officielle Bing, l’historique de zxyongyo/bing-daily-wallpaper, avec un instantané hors ligne intégré.",
            "Historical archive" => "Archive historique",
            "Project home" => "Page du projet",
            "Download folder hint" => "Laissez vide pour utiliser le dossier par défaut ; il sera créé lors de l’enregistrement si nécessaire.",
            "Theme hint" => "Un choix manuel ne sera pas remplacé par les changements du thème système.",
            "No display detected" => "Aucun écran configurable séparément ; les fonds seront synchronisés sur tous les écrans.",
            "Display target hint" => "Toutes les prochaines actions de définition du fond utiliseront cette cible.",
            "Auto exit hint" => "S’applique uniquement à l’exécution quotidienne ; le lancement manuel ne ferme pas l’application.",
            "Time selection hint" => "Sélectionnez séparément l’heure et les minutes.",
            "Automatic wallpaper hint" => "Le mode quotidien télécharge l’image du jour. Ajoutez des favoris avant d’utiliser le mode aléatoire.",
            "Windows scheduled wallpaper" => "Fond d’écran planifié par Windows",
            "Enable periodic task scheduler" => "Changer à l’ouverture de session et périodiquement",
            "Use latest Bing wallpaper for the first run each day" => "Utiliser le dernier fond Bing au premier lancement du jour",
            "Later wallpaper source" => "Source pour les lancements suivants du jour",
            "Repeat interval" => "Intervalle de répétition",
            "Periodic interval range hint" => "De 1 minute à 23 h 59 ; 00:00 est automatiquement remplacé par 00:01.",
            "Periodic task hint" => "La tâche s’exécute à l’ouverture de session de l’utilisateur actuel puis selon l’intervalle choisi. Une exécution manquée est lancée une fois lorsque le PC redevient disponible, sans le réveiller. L’application se ferme après chaque changement ; si les favoris sont vides, elle choisit dans l’historique. L’activation désactive l’ancien démarrage automatique.",
            "Startup disabled by periodic task hint" => "L’ancien démarrage automatique est indisponible pendant que la tâche périodique est active.",
            "Disable periodic task before enabling startup" => "Désactivez d’abord la tâche périodique avant d’activer l’ancien démarrage automatique.",
            "Periodic task enabled" => "Tâche périodique Windows activée",
            "Periodic task disabled" => "Tâche périodique Windows désactivée",
            "Periodic interval updated" => "Intervalle de la tâche périodique mis à jour",
            "Periodic settings saved" => "Paramètres périodiques enregistrés",
            "Periodic wallpaper source" => "Source du fond périodique",
            "Failed to enable periodic task" => "Impossible d’activer la tâche périodique Windows",
            "Failed to disable periodic task" => "Impossible de désactiver la tâche périodique Windows",
            "Failed to update periodic task" => "Impossible de mettre à jour la tâche périodique Windows",
            "Failed to save periodic settings" => "Impossible d’enregistrer les paramètres périodiques",
            "Failed to disable old startup" => "Impossible de désactiver l’ancien démarrage automatique",
            "Recent wallpapers" => "Fonds récents",
            "Favorites empty hint" => "Cliquez sur ❤ depuis l’accueil ou les archives pour ajouter un fond aux favoris.",
            "Quick download" => "Téléchargement rapide",
            "All history" => "Tout l’historique",
            "Current month" => "Mois actuel",
            "Download by date range" => "Télécharger par période",
            "Select a date range" => "Sélectionnez une période",
            "Download date range" => "Télécharger la période",
            "Select a month from the sidebar" => "Sélectionnez un mois à gauche",
            "Exit app title" => "Quitter la bibliothèque de fonds Bing ?",
            "Exit app prompt" => "L’exécution en arrière-plan est désactivée. Voulez-vous quitter ou réduire l’application dans la zone de notification ?",
            "Minimize tray hint" => "La réduction dans la zone de notification n’active pas le démarrage automatique. Activez-le dans les paramètres si nécessaire.",
            "Minimize to tray" => "Réduire dans la zone de notification",
            "Exit application" => "Quitter l’application",
            "Download wallpaper tooltip" => "Télécharger le fond haute résolution actuel",
            "Set wallpaper tooltip" => "Télécharger et appliquer le fond avec les réglages d’écran actuels",
            "Refresh list tooltip" => "Actualiser depuis les sources distantes ; la liste intégrée reste disponible hors connexion",
            "Already up to date" => "Vous utilisez déjà la dernière version",
            "Update check failed" => "Échec de la recherche de mises à jour",
            "New version prompt" => "La version v{version} est disponible (version actuelle v{current}). Mettre à jour maintenant ?",
            "Downloading version" => "Téléchargement de la version v{version}",
            "Speed" => "Vitesse",
            "Remaining" => "Temps restant",
            "Update restart hint" => "Après le téléchargement, l’application redémarrera automatiquement pour terminer la mise à jour. Ne la fermez pas.",
            "Wallpaper downloading..." => "Téléchargement du fond d’écran...",
            "Thumbnail generation failed" => "Échec de la création de la miniature. Actualisez pour réessayer",
            "Cannot read local image" => "Impossible de lire l’image locale",
            "Generating thumbnail..." => "Création de la miniature...",
            "Available date range" => "Période disponible : {start} à {end}. Les autres dates sont désactivées.",
            "Date range unavailable" => "La période sera disponible après le chargement de la liste des fonds.",
            "Download all history tooltip" => "Télécharger tous les fonds de l’historique",
            "Download current month tooltip" => "Télécharger les fonds du mois sélectionné à gauche",
            "Download favorites tooltip" => "Télécharger tous les fonds favoris",
            "Download range tooltip" => "Télécharger la période sélectionnée dans le calendrier",
            "Download progress" => "Progression du téléchargement",
            "Skipped" => "Ignorés",
            "Failed" => "Échecs",
            "Select all downloaded tooltip" => "Sélectionner ou désélectionner tous les fonds du dossier",
            "Delete selected tooltip" => "Supprimer les fonds locaux sélectionnés",
            "Rescan downloads tooltip" => "Analyser de nouveau le dossier de téléchargement",
            "Open downloads tooltip" => "Ouvrir le dossier de téléchargement dans l’Explorateur",
            "Downloaded empty hint" => "Les fonds téléchargés depuis l’accueil, les archives ou le téléchargement groupé apparaîtront ici",
            "Select local wallpaper tooltip" => "Sélectionner ce fond local pour la suppression groupée",
            "Set local wallpaper tooltip" => "Définir cette image locale comme fond d’écran",
            "Delete local wallpaper tooltip" => "Supprimer ce fichier de fond local",
            "Preview image tooltip" => "Afficher l’image haute résolution",
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

    #[test]
    fn tray_and_notification_keys_do_not_fall_back_to_english() {
        let keys = [
            "Open main window",
            "Run in background",
            "Change wallpaper daily",
            "Change once now",
            "Exit",
            "Already up to date",
            "Update check failed",
        ];

        for language in [
            LanguagePreference::Japanese,
            LanguagePreference::Korean,
            LanguagePreference::Russian,
            LanguagePreference::French,
        ] {
            for key in keys {
                assert_ne!(
                    language.t(key),
                    key,
                    "missing {language:?} translation: {key}"
                );
            }
        }
    }

    #[test]
    fn periodic_task_keys_are_translated_in_every_non_english_catalog() {
        let keys = [
            "Windows scheduled wallpaper",
            "Enable periodic task scheduler",
            "Use latest Bing wallpaper for the first run each day",
            "Later wallpaper source",
            "Repeat interval",
            "Periodic task hint",
            "Periodic task enabled",
            "Failed to update periodic task",
        ];

        for language in [
            LanguagePreference::SimplifiedChinese,
            LanguagePreference::Japanese,
            LanguagePreference::Korean,
            LanguagePreference::Russian,
            LanguagePreference::French,
        ] {
            for key in keys {
                assert_ne!(
                    language.t(key),
                    key,
                    "missing {language:?} translation: {key}"
                );
            }
        }
    }

    #[test]
    fn runtime_statuses_follow_the_selected_language() {
        assert_eq!(
            LanguagePreference::English.localize_status("已开启后台常驻（系统托盘图标已可用）"),
            "Setting enabled"
        );
        assert_eq!(
            LanguagePreference::Japanese.localize_status("检查更新失败: test"),
            "操作に失敗しました"
        );
        assert_eq!(
            LanguagePreference::Russian
                .localize_status("Automatic wallpaper source: Случайные из истории"),
            "Источник автоматических обоев: Случайные из истории"
        );
    }
}
