//! 主界面：左侧导航栏（主页 + 按年月分组）+ 右侧内容区域。
//!
//! 右侧内容区域有两种视图模式（见 [`ViewMode`]）：
//! - `Home`：默认打开的首页，展示最近的壁纸，铺满整个窗口的网格布局，
//!   支持鼠标滚轮触底自动加载更多（无限滚动）。
//! - `MonthDetail`：点击左侧导航栏中某个"年/月"条目后展示的旧版列表视图，
//!   只展示该月的壁纸。
//!
//! 点击左侧"主页"导航项会回到 `Home` 视图，但不会清空 `selected_key`，
//! 因此再次点击某个月份条目时仍会恢复到之前查看的那个月份。

use crate::downloader::Aria2Manager;
use crate::model::{group_by_month, MonthGroup, WallpaperEntry};
use crate::settings::{AppSettings, AutoWallpaperSource, ThemePreference, WallpaperTarget};
use crate::wallpaper_setter;
use chrono::{Datelike, Local, NaiveDate, Timelike};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants as _;
use gpui_component::checkbox::Checkbox;
use gpui_component::date_picker::{DatePicker, DatePickerState};
use gpui_component::dialog::DialogFooter;
use gpui_component::input::{Input, InputState};
use gpui_component::progress::Progress;
use gpui_component::scroll::ScrollableElement;
use gpui_component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
    SidebarToggleButton,
};
use gpui_component::*;
use gpui_component::{button::Button, theme::ThemeMode, Root, Theme};
use http_client::HttpClient;
use rand::seq::SliceRandom;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SetForegroundWindow, SetWindowPos, ShowWindow, SM_CXSCREEN, SM_CYSCREEN,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW, SW_SHOWNORMAL,
};

/// 软件版权/作者署名，展示于“关于”信息中。
const COPYRIGHT: &str = "© 2023-2026 小南瓜";

/// 首页网格视图每次展示/加载的壁纸数量。
const HOME_PAGE_SIZE: usize = 20;

/// 首页网格滚动到距离底部还剩多少像素时，自动加载下一页。
const LOAD_MORE_THRESHOLD: f32 = 300.0;

/// 右侧内容区域的当前视图模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    /// 默认首页：最近壁纸组成的网格，支持无限滚动加载更多。
    Home,
    /// 点击左侧“我的收藏”后展示收藏壁纸。
    Favorites,
    /// 点击左侧“下载中心 · 批量下载”后展示的批量下载页面。
    DownloadBatch,
    /// 点击左侧“下载中心 · 已下载的壁纸”后展示的本地已下载壁纸画廊。
    Downloaded,
    /// 点击左侧某个年/月条目后展示的旧版列表视图。
    MonthDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSection {
    DownloadDir,
    Appearance,
    WallpaperTarget,
    Automation,
    Maintenance,
}

/// 当前自动更新下载任务的实时进度快照。
///
/// 由后台异步任务（`run_update_download`）每 300ms 从 aria2 `tellStatus` 拉取一次并写入
/// `WallpaperLibrary::update_progress`（`Rc<RefCell<Option<_>>>`），同时 `cx.notify()` 触发重新
/// 渲染使弹窗读取最新值。
#[derive(Clone, Copy, Debug, Default)]
struct UpdateProgress {
    /// 已下载字节数。
    completed: u64,
    /// 总字节数（抓到第一次之前为 0，渲染时展示为“--”）。
    total: u64,
    /// 当前下载速度（字节/秒），时为 0 时 ETA 显示为“--”。
    speed: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct BatchProgress {
    completed: usize,
    total: usize,
    skipped: usize,
    failed: usize,
}

/// 应用主视图。
pub struct WallpaperLibrary {
    groups: Vec<MonthGroup>,
    /// 全部壁纸，按日期倒序（最新在前），用于首页网格视图；
    /// 在 [`WallpaperLibrary::set_entries`] 中随 `groups` 一并刷新。
    flat_entries: Vec<WallpaperEntry>,
    view_mode: ViewMode,
    selected_key: Option<String>,
    status: SharedString,
    aria2: Rc<RefCell<Option<Rc<Aria2Manager>>>>,
    http: Arc<dyn HttpClient>,
    /// 正在下载中的条目的实时进度（百分比 0.0~100.0），按日期索引。
    progress: HashMap<NaiveDate, f32>,
    /// 用户收藏的壁纸日期集合。
    favorites: HashSet<NaiveDate>,
    /// 当前批量下载任务进度；`None` 表示当前没有批量下载。
    batch_progress: Option<BatchProgress>,
    /// 当前可单独设置桌面壁纸的显示器列表。
    monitors: Vec<wallpaper_setter::MonitorInfo>,
    /// 当前自动更新下载任务的实时进度（字节级）；`None` 表示当前没有在下载。
    /// 使用 `Rc<RefCell<_>>` 是为了让“下载进度弹窗”的 `build` 闭包能在每次重新渲染时
    /// 读到最新值，而不需要在弹窗 builder 内部反向 `.read()` 主视图（后者会触发
    /// GPUI 的 entity 重入锁定 panic，见 AGENTS.md §12.3）。
    update_progress: Rc<RefCell<Option<UpdateProgress>>>,
    /// 首页网格当前已展示的壁纸数量（初始 [`HOME_PAGE_SIZE`] 张，滚动到底部后递增）。
    home_loaded_count: usize,
    /// 首页网格滚动容器的滚动状态句柄，用于判断是否已接近底部。
    home_scroll_handle: ScrollHandle,
    /// 侧边导航栏是否处于折叠（仅图标）状态。
    sidebar_collapsed: bool,
    /// 设置浮层是否展开（左下角，类似抖音侧边栏设置菜单）。
    settings_panel_open: bool,
    /// 设置浮层当前展开的分组；`None` 表示只显示分组标题。
    settings_section: Option<SettingsSection>,
    /// 持久化的应用设置。
    settings: AppSettings,
    /// 设置面板中"下载路径"输入框的状态。
    settings_dir_input: Entity<InputState>,
    /// 批量下载日期范围选择器（点击展开日历直接选日期，不需要手动输入数字）。
    batch_range_picker: Entity<DatePickerState>,
    /// 当前已从云端/缓存加载到的壁纸日期范围（最早日期、最新日期）。
    /// `DatePickerState::disabled_matcher` 需要 `'static + Send + Sync` 的闭包，
    /// 因此用 `Arc<Mutex<_>>` 让闭包能随列表刷新读取最新范围。
    batch_date_limits: Arc<Mutex<Option<(NaiveDate, NaiveDate)>>>,
    /// 已下载壁纸画廊中当前选中的文件路径集合，用于批量删除。
    downloaded_selected: HashSet<std::path::PathBuf>,
}

impl WallpaperLibrary {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings = AppSettings::load();
        settings.startup_enabled = crate::startup::is_enabled();
        let initial_dir_text = settings
            .download_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let default_dir_display = crate::paths::default_wallpapers_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let settings_dir_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("默认: {default_dir_display}"))
                .default_value(initial_dir_text)
        });
        let batch_date_limits = Arc::new(Mutex::new(None::<(NaiveDate, NaiveDate)>));
        let limits_for_picker = batch_date_limits.clone();
        let batch_range_picker = cx.new(|cx| {
            DatePickerState::range(window, cx)
                .date_format("%Y年%m月%d日")
                .disabled_matcher(move |date: &NaiveDate| {
                    limits_for_picker
                        .lock()
                        .ok()
                        .and_then(|limits| *limits)
                        .is_some_and(|(earliest, latest)| *date < earliest || *date > latest)
                })
        });
        let monitors = wallpaper_setter::list_monitors().unwrap_or_default();

        Self {
            groups: Vec::new(),
            flat_entries: Vec::new(),
            view_mode: ViewMode::Home,
            selected_key: None,
            status: "正在加载壁纸列表...".into(),
            aria2: Rc::new(RefCell::new(None)),
            http: cx.http_client(),
            progress: HashMap::new(),
            favorites: crate::favorites::load(),
            batch_progress: None,
            monitors,
            update_progress: Rc::new(RefCell::new(None)),
            home_loaded_count: HOME_PAGE_SIZE,
            home_scroll_handle: ScrollHandle::new(),
            sidebar_collapsed: false,
            settings_panel_open: false,
            settings_section: None,
            settings,
            settings_dir_input,
            batch_range_picker,
            batch_date_limits,
            downloaded_selected: HashSet::new(),
        }
    }

    /// 导出内部持有的 aria2 管理器共享句柄，供应用退出时优雅关闭使用（见 `main.rs`）。
    pub fn aria2_handle(&self) -> Rc<RefCell<Option<Rc<Aria2Manager>>>> {
        self.aria2.clone()
    }

    /// 使用最新抓取到的壁纸条目刷新界面状态。
    pub fn set_entries(&mut self, entries: Vec<WallpaperEntry>, cx: &mut Context<Self>) {
        let date_limits =
            entries
                .iter()
                .map(|entry| entry.date)
                .fold(None, |acc, date| match acc {
                    None => Some((date, date)),
                    Some((earliest, latest)) => Some((earliest.min(date), latest.max(date))),
                });
        if let Ok(mut limits) = self.batch_date_limits.lock() {
            *limits = date_limits;
        }
        if let Some((earliest, latest)) = date_limits {
            self.batch_range_picker.update(cx, |picker, cx| {
                picker.set_year_range((earliest.year(), latest.year() + 1), cx);
            });
        }

        self.groups = group_by_month(&entries);
        if self.selected_key.is_none() {
            self.selected_key = self.groups.first().map(|g| g.key.clone());
        }
        self.flat_entries = self
            .groups
            .iter()
            .flat_map(|g| g.entries.iter().cloned())
            .collect();
        self.status = format!("共 {} 张壁纸", entries.len()).into();
        cx.notify();
    }

    pub fn set_status(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = message.into();
        cx.notify();
    }

    fn selected_group(&self) -> Option<&MonthGroup> {
        let key = self.selected_key.as_ref()?;
        self.groups.iter().find(|g| &g.key == key)
    }

    /// 按年份对月份分组进行二级归类，供侧边栏渲染。
    fn years(&self) -> Vec<(i32, Vec<&MonthGroup>)> {
        let mut years: Vec<i32> = self.groups.iter().map(|g| g.year).collect();
        years.sort_unstable();
        years.dedup();
        years.reverse();
        years
            .into_iter()
            .map(|year| {
                let months: Vec<&MonthGroup> =
                    self.groups.iter().filter(|g| g.year == year).collect();
                (year, months)
            })
            .collect()
    }

    fn select(&mut self, key: String, cx: &mut Context<Self>) {
        self.selected_key = Some(key);
        self.view_mode = ViewMode::MonthDetail;
        cx.notify();
    }

    fn favorite_entries(&self) -> Vec<WallpaperEntry> {
        self.flat_entries
            .iter()
            .filter(|entry| self.favorites.contains(&entry.date))
            .cloned()
            .collect()
    }

    fn toggle_favorite(&mut self, date: NaiveDate, cx: &mut Context<Self>) {
        let added = if self.favorites.remove(&date) {
            false
        } else {
            self.favorites.insert(date);
            true
        };

        match crate::favorites::save(&self.favorites) {
            Ok(()) => {
                let action = if added {
                    "已加入收藏"
                } else {
                    "已取消收藏"
                };
                self.set_status(format!("{action}: {date}"), cx);
            }
            Err(err) => self.set_status(format!("保存收藏失败: {err}"), cx),
        }
    }

    fn wallpaper_target_label(&self) -> String {
        match &self.settings.wallpaper_target {
            WallpaperTarget::All => "全部显示器".to_string(),
            WallpaperTarget::Monitor(id) => self
                .monitors
                .iter()
                .find(|monitor| &monitor.id == id)
                .map(|monitor| monitor.label.clone())
                .unwrap_or_else(|| "选定显示器".to_string()),
        }
    }

    pub fn toggle_startup_enabled(&mut self, cx: &mut Context<Self>) {
        let enabled = !self.settings.startup_enabled;
        match crate::startup::set_enabled(enabled) {
            Ok(()) => {
                self.settings.startup_enabled = enabled;
                let _ = self.settings.save();
                self.set_status(
                    if enabled {
                        "已开启开机自启"
                    } else {
                        "已关闭开机自启"
                    },
                    cx,
                );
            }
            Err(err) => self.set_status(format!("修改开机自启失败: {err}"), cx),
        }
    }

    pub fn toggle_background_resident_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings.background_resident_enabled = !self.settings.background_resident_enabled;
        let _ = self.settings.save();
        self.set_status(
            if self.settings.background_resident_enabled {
                "已开启后台常驻（系统托盘图标已可用）"
            } else {
                "已关闭后台常驻"
            },
            cx,
        );
    }

    pub fn toggle_auto_wallpaper_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_enabled = !self.settings.auto_wallpaper_enabled;
        let _ = self.settings.save();
        self.set_status(
            if self.settings.auto_wallpaper_enabled {
                "已开启每日自动壁纸"
            } else {
                "已关闭每日自动壁纸"
            },
            cx,
        );
    }

    fn set_auto_wallpaper_source(&mut self, source: AutoWallpaperSource, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_source = source;
        let _ = self.settings.save();
        self.set_status(format!("每日自动壁纸来源已设为{}", source.label()), cx);
    }

    fn set_auto_wallpaper_time(&mut self, hour: u8, minute: u8, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_hour = hour.min(23);
        self.settings.auto_wallpaper_minute = minute.min(59);
        let _ = self.settings.save();
        self.set_status(
            format!(
                "每日自动壁纸时间已设为 {:02}:{:02}",
                hour.min(23),
                minute.min(59)
            ),
            cx,
        );
    }

    pub fn trigger_auto_wallpaper_now(&mut self, cx: &mut Context<Self>) {
        self.run_auto_wallpaper(false, cx);
    }

    pub fn should_close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.settings.background_resident_enabled {
            hide_window_to_tray(window);
            self.set_status("已最小化到系统托盘，右键托盘图标可退出", cx);
            false
        } else {
            self.open_close_choice_dialog(window, cx);
            false
        }
    }

    fn open_close_choice_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_dialog(cx, move |dialog, _window, _cx| {
            dialog
                .title("退出必应每日壁纸库？")
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_2()
                        .p_4()
                        .child(div().text_sm().child("后台常驻当前未开启。你想直接退出程序，还是仅最小化到系统托盘继续后台运行？"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(_cx.theme().muted_foreground)
                                .child("选择“最小化到托盘”不会自动开启开机自启；如需开机后台运行，请在设置里开启开机自启。"),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("close-choice-minimize")
                                .label("最小化到托盘")
                                .outline()
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                    hide_window_to_tray(window);
                                }),
                        )
                        .child(
                            Button::new("close-choice-exit")
                                .label("退出程序")
                                .danger()
                                .on_click(|_, _window, cx| {
                                    cx.quit();
                                }),
                        ),
                )
        });
    }

    pub fn check_scheduled_wallpaper(&mut self, cx: &mut Context<Self>) {
        if !self.settings.auto_wallpaper_enabled {
            return;
        }
        let now = Local::now();
        let today = now.date_naive();
        if self.settings.last_auto_wallpaper_date == Some(today) {
            return;
        }
        let current_minutes = now.hour() as u16 * 60 + now.minute() as u16;
        let scheduled_minutes = self.settings.auto_wallpaper_hour as u16 * 60
            + self.settings.auto_wallpaper_minute as u16;
        if current_minutes >= scheduled_minutes {
            self.run_auto_wallpaper(true, cx);
        }
    }

    fn run_auto_wallpaper(&mut self, mark_today: bool, cx: &mut Context<Self>) {
        let entry = match self.settings.auto_wallpaper_source {
            AutoWallpaperSource::Latest => self.flat_entries.first().cloned(),
            AutoWallpaperSource::RandomAll => {
                self.flat_entries.choose(&mut rand::thread_rng()).cloned()
            }
            AutoWallpaperSource::RandomFavorites => {
                let favorites = self.favorite_entries();
                favorites.choose(&mut rand::thread_rng()).cloned()
            }
        };

        let Some(entry) = entry else {
            self.set_status("没有可用于自动更换的壁纸，请等待列表加载或先添加收藏", cx);
            return;
        };

        if mark_today {
            self.settings.last_auto_wallpaper_date = Some(Local::now().date_naive());
            let _ = self.settings.save();
        }
        self.set_status(
            format!(
                "自动壁纸：正在使用{} - {}",
                self.settings.auto_wallpaper_source.label(),
                entry.date
            ),
            cx,
        );
        self.set_as_wallpaper(entry, cx);
    }

    fn show_settings_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
        if self.settings_section != Some(section) {
            self.settings_section = Some(section);
            cx.notify();
        }
    }

    fn set_wallpaper_target(&mut self, target: WallpaperTarget, cx: &mut Context<Self>) {
        self.settings.wallpaper_target = target;
        if let Err(err) = self.settings.save() {
            self.set_status(format!("保存显示器设置失败: {err}"), cx);
            return;
        }
        self.set_status(
            format!("设置壁纸目标已切换为{}", self.wallpaper_target_label()),
            cx,
        );
        cx.notify();
    }

    fn refresh_monitors(&mut self, cx: &mut Context<Self>) {
        match wallpaper_setter::list_monitors() {
            Ok(monitors) => {
                let count = monitors.len();
                self.monitors = monitors;
                if let WallpaperTarget::Monitor(id) = &self.settings.wallpaper_target {
                    if !self.monitors.iter().any(|monitor| &monitor.id == id) {
                        self.settings.wallpaper_target = WallpaperTarget::All;
                        let _ = self.settings.save();
                    }
                }
                self.set_status(format!("已刷新显示器列表：{count} 个"), cx);
                cx.notify();
            }
            Err(err) => self.set_status(format!("刷新显示器列表失败: {err}"), cx),
        }
    }

    fn start_date_range_batch_download(
        &mut self,
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
        cx: &mut Context<Self>,
    ) {
        if start.is_none() && end.is_none() {
            self.set_status("请先在日历中选择批量下载的日期范围", cx);
            return;
        }
        if let (Some(start), Some(end)) = (start, end) {
            if start > end {
                self.set_status("开始日期不能晚于结束日期", cx);
                return;
            }
        }
        if let Some((earliest, latest)) = self
            .batch_date_limits
            .lock()
            .ok()
            .and_then(|limits| *limits)
        {
            if start.is_some_and(|date| date < earliest) || end.is_some_and(|date| date > latest) {
                self.set_status(
                    format!(
                        "请选择 {} 至 {} 之间的日期",
                        format_date_cn(earliest),
                        format_date_cn(latest)
                    ),
                    cx,
                );
                return;
            }
        }

        let entries: Vec<WallpaperEntry> = self
            .flat_entries
            .iter()
            .filter(|entry| start.is_none_or(|date| entry.date >= date))
            .filter(|entry| end.is_none_or(|date| entry.date <= date))
            .cloned()
            .collect();
        let label = match (start, end) {
            (Some(start), Some(end)) => format!("日期范围 {start} 至 {end}"),
            (Some(start), None) => format!("日期范围 {start} 至今"),
            (None, Some(end)) => format!("日期范围 截止 {end}"),
            (None, None) => unreachable!(),
        };
        self.start_batch_download(label, entries, cx);
    }

    /// 首页网格滚动条即将触底时，追加下一页壁纸（无限滚动）。
    fn maybe_load_more_home(&mut self, cx: &mut Context<Self>) {
        let total = self.flat_entries.len();
        if self.home_loaded_count >= total {
            return;
        }
        let offset = self.home_scroll_handle.offset();
        let max_offset = self.home_scroll_handle.max_offset();
        // `offset.y` 在向下滚动时为负值，`max_offset.y` 为可滚动的总距离；
        // 二者之和即"距离底部还剩多少像素"。
        let remaining = max_offset.y + offset.y;
        if remaining <= px(LOAD_MORE_THRESHOLD) {
            self.home_loaded_count = (self.home_loaded_count + HOME_PAGE_SIZE).min(total);
            cx.notify();
        }
    }

    fn start_download(&mut self, entry: WallpaperEntry, cx: &mut Context<Self>) {
        let aria2 = self.aria2.clone();
        let http = self.http.clone();
        let date = entry.date;
        self.status = format!("正在下载 {} ...", entry.date).into();
        self.progress.insert(date, 0.0);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = run_download(&aria2, &http, &entry, &this, cx).await;
            let _ = this.update(cx, |this, cx| {
                this.progress.remove(&date);
                match result {
                    Ok(path) => this.set_status(format!("已下载完成: {}", path.display()), cx),
                    Err(err) => this.set_status(format!("下载失败: {err}"), cx),
                }
            });
        })
        .detach();
    }

    fn start_batch_download(
        &mut self,
        label: impl Into<String>,
        entries: Vec<WallpaperEntry>,
        cx: &mut Context<Self>,
    ) {
        let label = label.into();
        if entries.is_empty() {
            self.set_status(format!("{label}没有可下载的壁纸"), cx);
            return;
        }
        if self.batch_progress.is_some() {
            self.set_status("已有批量下载任务正在进行，请等待完成", cx);
            return;
        }

        let total = entries.len();
        let aria2 = self.aria2.clone();
        let http = self.http.clone();
        self.batch_progress = Some(BatchProgress {
            total,
            ..Default::default()
        });
        self.set_status(format!("开始批量下载{label}: 共 {total} 张"), cx);

        cx.spawn(async move |this, cx| {
            let mut completed = 0usize;
            let mut skipped = 0usize;
            let mut failed = 0usize;
            let mut tasks = Vec::new();

            let manager = match ensure_aria2(&aria2, &http).await {
                Ok(manager) => manager,
                Err(err) => {
                    let _ = this.update(cx, |this, cx| {
                        this.batch_progress = None;
                        this.set_status(format!("批量下载{label}失败: {err}"), cx);
                    });
                    return;
                }
            };

            for entry in entries {
                let file_name = entry.file_name();
                let existing = crate::paths::wallpapers_dir().map(|dir| dir.join(&file_name));
                if existing.as_ref().is_ok_and(|path| path.exists()) {
                    completed += 1;
                    skipped += 1;
                    continue;
                }

                let date = entry.date;
                match manager.add_uri(&entry.url, &file_name).await {
                    Ok(gid) => {
                        tasks.push((gid, date, false));
                        let _ = this.update(cx, |this, cx| {
                            this.progress.insert(date, 0.0);
                            this.batch_progress = Some(BatchProgress {
                                completed,
                                total,
                                skipped,
                                failed,
                            });
                            this.set_status(
                                format!("批量下载{label}: 已提交 {} 个并发任务", tasks.len()),
                                cx,
                            );
                        });
                    }
                    Err(_) => {
                        completed += 1;
                        failed += 1;
                    }
                }
            }

            while tasks.iter().any(|(_, _, done)| !*done) {
                let mut progress_updates = Vec::new();
                for (gid, date, done) in tasks.iter_mut() {
                    if *done {
                        continue;
                    }
                    match manager.tell_status(gid).await {
                        Ok(status) => {
                            let state = status
                                .get("status")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("");
                            let completed_len: u64 = status
                                .get("completedLength")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            let total_len: u64 = status
                                .get("totalLength")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(0);
                            let percent = if total_len > 0 {
                                completed_len as f32 / total_len as f32 * 100.0
                            } else {
                                0.0
                            };
                            progress_updates.push((*date, percent));

                            match state {
                                "complete" => {
                                    *done = true;
                                    completed += 1;
                                }
                                "error" | "removed" => {
                                    *done = true;
                                    completed += 1;
                                    failed += 1;
                                }
                                _ => {}
                            }
                        }
                        Err(_) => {
                            *done = true;
                            completed += 1;
                            failed += 1;
                        }
                    }
                }

                let finished_dates: Vec<_> = tasks
                    .iter()
                    .filter_map(|(_, date, done)| (*done).then_some(*date))
                    .collect();
                let _ = this.update(cx, |this, cx| {
                    for (date, percent) in progress_updates {
                        this.progress.insert(date, percent);
                    }
                    for date in finished_dates {
                        this.progress.remove(&date);
                    }
                    this.batch_progress = Some(BatchProgress {
                        completed,
                        total,
                        skipped,
                        failed,
                    });
                    this.set_status(
                        format!(
                            "批量下载{label}: {completed}/{total}（跳过 {skipped}，失败 {failed}）"
                        ),
                        cx,
                    );
                });

                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
            }

            let _ = this.update(cx, |this, cx| {
                this.batch_progress = None;
                this.set_status(
                    format!("批量下载{label}完成：共 {total}，跳过 {skipped}，失败 {failed}"),
                    cx,
                );
            });
        })
        .detach();
    }

    fn set_as_wallpaper(&mut self, entry: WallpaperEntry, cx: &mut Context<Self>) {
        let aria2 = self.aria2.clone();
        let http = self.http.clone();
        let date = entry.date;
        let target = self.settings.wallpaper_target.clone();
        let target_label = self.wallpaper_target_label();
        self.status = format!("正在将 {} 设置为{}壁纸...", entry.date, target_label).into();
        self.progress.insert(date, 0.0);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = run_download(&aria2, &http, &entry, &this, cx).await;
            let outcome = match result {
                Ok(path) => match &target {
                    WallpaperTarget::All => wallpaper_setter::set_wallpaper_for_all_monitors(&path),
                    WallpaperTarget::Monitor(monitor_id) => {
                        wallpaper_setter::set_wallpaper_for_monitor(&path, monitor_id)
                    }
                },
                Err(err) => Err(err),
            };
            let _ = this.update(cx, |this, cx| {
                this.progress.remove(&date);
                match outcome {
                    Ok(()) => {
                        this.set_status(format!("已将 {} 设置为{}壁纸", date, target_label), cx)
                    }
                    Err(err) => this.set_status(format!("设置壁纸失败: {err}"), cx),
                }
            });
        })
        .detach();
    }

    /// 直接将本地已下载的壁纸文件设置为桌面壁纸（无需重新下载，因此同步执行即可）。
    fn set_local_file_as_wallpaper(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let target = self.settings.wallpaper_target.clone();
        let target_label = self.wallpaper_target_label();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let outcome = match &target {
            WallpaperTarget::All => wallpaper_setter::set_wallpaper_for_all_monitors(&path),
            WallpaperTarget::Monitor(monitor_id) => {
                wallpaper_setter::set_wallpaper_for_monitor(&path, monitor_id)
            }
        };
        match outcome {
            Ok(()) => self.set_status(format!("已将 {name} 设置为{target_label}壁纸"), cx),
            Err(err) => self.set_status(format!("设置壁纸失败: {err}"), cx),
        }
    }

    /// 删除单个已下载的壁纸文件，并同步从批量选中集合中移除。
    fn delete_downloaded_file(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.downloaded_selected.remove(&path);
        match std::fs::remove_file(&path) {
            Ok(()) => self.set_status(format!("已删除 {name}"), cx),
            Err(err) => self.set_status(format!("删除 {name} 失败: {err}"), cx),
        }
    }

    /// 批量删除已选中的已下载壁纸文件。
    fn delete_selected_downloaded(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<std::path::PathBuf> = self.downloaded_selected.drain().collect();
        if paths.is_empty() {
            self.set_status("请先勾选需要删除的壁纸", cx);
            return;
        }
        let total = paths.len();
        let mut deleted = 0usize;
        let mut failed = 0usize;
        for path in paths {
            match std::fs::remove_file(&path) {
                Ok(()) => deleted += 1,
                Err(_) => failed += 1,
            }
        }
        self.set_status(
            format!("批量删除完成：共 {total}，成功 {deleted}，失败 {failed}"),
            cx,
        );
    }

    /// 应用新的下载目录设置：写入磁盘，并（若 aria2 已在运行）通过
    /// `aria2.changeGlobalOption` 实时生效，影响之后新提交的下载任务。
    fn apply_download_dir(&mut self, path_str: String, cx: &mut Context<Self>) {
        let trimmed = path_str.trim();
        self.settings.download_dir = if trimmed.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(trimmed))
        };

        if let Err(err) = self.settings.save() {
            self.set_status(format!("保存设置失败: {err}"), cx);
            return;
        }

        match self.settings.effective_download_dir() {
            Ok(dir) => {
                self.set_status(format!("已保存下载路径: {}", dir.display()), cx);
                let manager = self.aria2.borrow().clone();
                if let Some(manager) = manager {
                    cx.spawn(async move |_this, _cx| {
                        let _ = manager.change_download_dir(&dir).await;
                    })
                    .detach();
                }
            }
            Err(err) => self.set_status(format!("下载路径无效: {err}"), cx),
        }
    }

    fn set_theme_preference(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings.theme_preference = preference;
        if let Err(err) = self.settings.save() {
            self.set_status(format!("保存主题设置失败: {err}"), cx);
            return;
        }

        match preference {
            ThemePreference::System => Theme::sync_system_appearance(Some(window), cx),
            ThemePreference::Light => Theme::change(ThemeMode::Light, Some(window), cx),
            ThemePreference::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
        }
        self.set_status(format!("已切换为{}", preference.label()), cx);
        cx.notify();
    }

    fn clear_cache(&mut self, cx: &mut Context<Self>) {
        match crate::paths::cache_file() {
            Ok(path) => match std::fs::remove_file(&path) {
                Ok(()) => self.set_status("已清空本地壁纸缓存，下次刷新会重新写入", cx),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    self.set_status("本地壁纸缓存已经为空", cx);
                }
                Err(err) => self.set_status(format!("清空缓存失败: {err}"), cx),
            },
            Err(err) => self.set_status(format!("定位缓存文件失败: {err}"), cx),
        }
    }

    /// 打开"预览图片"对话框：展示原始高清大图，底部提供下载/设为壁纸按钮。
    ///
    /// 注意：`downloading` 必须在调用 `window.open_dialog` **之前**，从 `&self` 同步快照
    /// 一次，而不能在对话框的 builder 闭包内部通过 `view.read(cx)` 读取——因为
    /// `render_dialog_layer` 是在 `WallpaperLibrary::render` 自身的渲染过程中被调用的，
    /// 此时本 Entity 正处于"正在被更新"状态，重入读取会触发 GPUI 的
    /// `cannot read ... while it is already being updated` panic（应用直接崩溃）。
    fn open_preview_dialog(
        &self,
        entry: WallpaperEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity();
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        let title = entry.title.clone();
        let url = entry.url.clone();
        let downloading = self.progress.contains_key(&entry.date);

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_for_dl = view.clone();
            let view_for_wall = view.clone();
            let entry_for_dl = entry.clone();
            let entry_for_wall = entry.clone();

            dialog
                .title(date_str.clone())
                .w(px(860.))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(img(url.clone()).w(px(800.)).h(px(450.)).rounded(px(6.)))
                        .child(div().text_sm().child(title.clone())),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("preview-download")
                                .label("下载")
                                .disabled(downloading)
                                .on_click(move |_, window, cx| {
                                    let entry = entry_for_dl.clone();
                                    view_for_dl.update(cx, |this, cx| {
                                        this.start_download(entry, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("preview-set-wallpaper")
                                .label("设为桌面壁纸")
                                .primary()
                                .disabled(downloading)
                                .on_click(move |_, window, cx| {
                                    let entry = entry_for_wall.clone();
                                    view_for_wall.update(cx, |this, cx| {
                                        this.set_as_wallpaper(entry, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    /// 检查 GitHub 上是否有新版本。`manual` 为 `true` 时表示由用户主动点击
    /// “检查更新”触发，会用通知提示“已是最新版本”/“检查失败”；为 `false` 表示
    /// 启动时自动静默检查，未发现更新或检查失败时不打扰用户。
    fn check_for_updates(&mut self, manual: bool, window: &mut Window, cx: &mut Context<Self>) {
        let http = self.http.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = crate::updater::check_for_update(http).await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(Some(release)) => this.open_update_dialog(release, window, cx),
                Ok(None) => {
                    if manual {
                        window.push_notification("当前已是最新版本", cx);
                    }
                }
                Err(err) => {
                    if manual {
                        window.push_notification(format!("检查更新失败: {err}"), cx);
                    }
                }
            });
        })
        .detach();
    }

    /// 展示“发现新版本”对话框，供用户选择立即更新或稍后再说。
    pub fn open_update_dialog(
        &self,
        release: crate::updater::ReleaseInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_for_update = view.clone();
            let release_for_link = release.clone();
            let release_for_update = release.clone();
            let version = release.version.clone();

            dialog
                .title("发现新版本")
                .w(px(440.))
                .child(
                    v_flex()
                        .gap_2()
                        .p_4()
                        .child(div().text_sm().child(format!(
                            "发现新版本 v{version}（当前 v{}），是否立即下载并更新？",
                            crate::updater::CURRENT_VERSION
                        )))
                        .child(
                            Button::new("update-view-release")
                                .label("查看更新内容")
                                .link()
                                .on_click(move |_, _, cx| {
                                    cx.open_url(&release_for_link.html_url);
                                }),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("update-later")
                                .label("稍后再说")
                                .outline()
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("update-now")
                                .label("立即更新")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let release = release_for_update.clone();
                                    view_for_update.update(cx, |this, cx| {
                                        this.start_update(release, window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    /// 下载新版本并启动“替换 + 重启”辅助脚本，随后退出当前进程。
    ///
    /// 下载本身通过项目内置的 `aria2c.exe`（而不是 `http_client` 的直接 GET）完成：
    /// GitHub 的 release asset 地址会 302 重定向到一个带签名参数的
    /// `release-assets.githubusercontent.com` 地址，reqwest 处理这个重定向链经常直接返回
    /// 400 Bad Request；aria2 则处理得很好，同时还能提供下载进度/已下字节/总字节/速度
    /// 等信息供弹窗实时展示。
    fn start_update(
        &mut self,
        release: crate::updater::ReleaseInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 先关闭“发现新版本”弹窗，避免与新弹出的“下载中”弹窗重叠。
        window.close_dialog(cx);

        // 初始化进度为全零，并弹出一个新的“正在下载新版本”弹窗（引用同一份
        // `update_progress` 共享句柄，将在后台下载进度更新时自动重新渲染）。
        *self.update_progress.borrow_mut() = Some(UpdateProgress::default());
        self.set_status(format!("正在下载新版本 v{} ...", release.version), cx);
        self.open_update_progress_dialog(release.clone(), window, cx);

        let aria2 = self.aria2.clone();
        let http = self.http.clone();
        let progress = self.update_progress.clone();

        cx.spawn(async move |this, cx| {
            let result = run_update_download(&aria2, &http, &release, &progress, &this, cx).await;
            let _ = this.update_in(cx, |this, window, cx| {
                *this.update_progress.borrow_mut() = None;
                window.close_dialog(cx);
                match result {
                    Ok(path) => match crate::updater::spawn_relaunch(&path) {
                        Ok(()) => {
                            this.set_status("下载完成，即将重启以完成更新...", cx);
                            cx.quit();
                        }
                        Err(err) => {
                            this.set_status(format!("启动更新程序失败: {err}"), cx);
                        }
                    },
                    Err(err) => {
                        this.set_status(format!("下载新版本失败: {err}"), cx);
                    }
                }
            });
        })
        .detach();
    }

    /// 弹出“正在下载新版本”对话框：展示实时进度条 + 已下/总字节 + 百分比 + 速度 + 剩余时间。
    ///
    /// 对话框 builder 闭包每次重新渲染时都会重新读取共享的 `Rc<RefCell<Option<UpdateProgress>>>`
    /// 拿到最新值，而后台下载任务每次写入新进度后会 `cx.notify()` 触发一次重渲染，
    /// 从而实现实时更新。**不能**在 builder 内部反向 `.read()` `WallpaperLibrary` 自身（会触发
    /// GPUI 的 entity 重入锁定 panic，见 AGENTS.md §12.3），因此不能直接从 `self.progress` 读取。
    fn open_update_progress_dialog(
        &self,
        release: crate::updater::ReleaseInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let progress_handle = self.update_progress.clone();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let snapshot = progress_handle.borrow().unwrap_or_default();
            let (completed, total, speed) = (snapshot.completed, snapshot.total, snapshot.speed);
            let percent = if total > 0 {
                (completed as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            };
            let size_text = if total > 0 {
                format!("{} / {}", format_bytes(completed), format_bytes(total))
            } else {
                format!("{} / --", format_bytes(completed))
            };
            let speed_text = if speed > 0 {
                format!("{}/s", format_bytes(speed))
            } else {
                "--".to_string()
            };
            let eta_text = if speed > 0 && total > completed {
                format_duration((total - completed) / speed)
            } else {
                "--".to_string()
            };

            dialog
                .title(format!("正在下载新版本 v{}", release.version))
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(Progress::new("update-progress").value(percent))
                        .child(
                            h_flex()
                                .justify_between()
                                .child(div().text_sm().child(size_text))
                                .child(div().text_sm().child(format!("{percent:.1}%"))),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .opacity(0.7)
                                        .child(format!("速度：{speed_text}")),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .opacity(0.7)
                                        .child(format!("剩余：{eta_text}")),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .opacity(0.6)
                                .child("下载完成后应用会自动重启完成更新，请勿关闭。"),
                        ),
                )
        });
    }
}

/// 打开“关于”对话框：展示版本信息、致谢开源项目与本项目的仓库链接。
fn open_about_dialog(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, cx| {
        dialog.title("关于").w(px(460.)).child(
            v_flex()
                .gap_4()
                .p_4()
                .child(
                    v_flex()
                        .gap_1()
                        .child(div().text_lg().font_bold().child(crate::paths::APP_NAME))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("每日自动获取、浏览、下载并设置 Bing 每日壁纸"),
                        ),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .p_3()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("当前版本"),
                                )
                                .child(
                                    div()
                                        .font_bold()
                                        .child(format!("v{}", crate::updater::CURRENT_VERSION)),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("版权信息"),
                                )
                                .child(div().font_bold().child(COPYRIGHT)),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().font_bold().child("开源与致谢"))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(
                                "壁纸数据来源于 niumoo/bing-wallpaper，本软件项目托管于 GitHub。",
                            ),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("about-bing-wallpaper")
                                        .label("数据源项目")
                                        .outline()
                                        .on_click(|_, _, cx| {
                                            cx.open_url("https://github.com/niumoo/bing-wallpaper");
                                        }),
                                )
                                .child(
                                    Button::new("about-repo")
                                        .label("项目主页")
                                        .outline()
                                        .on_click(|_, _, cx| {
                                            cx.open_url(crate::updater::REPO_HTML_URL);
                                        }),
                                ),
                        ),
                ),
        )
    });
}

/// 打开系统资源管理器窗口指向给定路径；路径为空时回退到默认下载目录。
/// 静默忽略失败（例如路径非法），避免阻塞设置面板的其他操作。
fn open_in_explorer(path: &str) {
    let trimmed = path.trim();
    let target = if trimmed.is_empty() {
        crate::paths::default_wallpapers_dir().unwrap_or_default()
    } else {
        std::path::PathBuf::from(trimmed)
    };
    let _ = std::fs::create_dir_all(&target);
    let _ = std::process::Command::new("explorer").arg(&target).spawn();
}

async fn ensure_aria2(
    aria2: &Rc<RefCell<Option<Rc<Aria2Manager>>>>,
    http: &Arc<dyn HttpClient>,
) -> anyhow::Result<Rc<Aria2Manager>> {
    if let Some(existing) = aria2.borrow().clone() {
        return Ok(existing);
    }
    let manager = Rc::new(Aria2Manager::start(http.clone()).await?);
    *aria2.borrow_mut() = Some(manager.clone());
    Ok(manager)
}

/// 下载指定壁纸并实时上报进度（通过定期轮询 aria2 的 `tell_status` 实现）。
async fn run_download(
    aria2: &Rc<RefCell<Option<Rc<Aria2Manager>>>>,
    http: &Arc<dyn HttpClient>,
    entry: &WallpaperEntry,
    this: &WeakEntity<WallpaperLibrary>,
    cx: &mut AsyncApp,
) -> anyhow::Result<std::path::PathBuf> {
    let manager = ensure_aria2(aria2, http).await?;
    let filename = entry.file_name();
    let date = entry.date;
    let gid = manager.add_uri(&entry.url, &filename).await?;

    loop {
        let status = manager.tell_status(&gid).await?;
        let state = status
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();

        let completed: f64 = status
            .get("completedLength")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let total: f64 = status
            .get("totalLength")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let percent = if total > 0.0 {
            ((completed / total) * 100.0) as f32
        } else {
            0.0
        };
        let _ = this.update(cx, |this, cx| {
            this.progress.insert(date, percent);
            cx.notify();
        });

        match state.as_str() {
            "complete" => {
                let dir = crate::paths::wallpapers_dir()?;
                return Ok(dir.join(&filename));
            }
            "error" => {
                let msg = status
                    .get("errorMessage")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("未知错误")
                    .to_string();
                anyhow::bail!("下载失败: {msg}");
            }
            _ => {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(300))
                    .await;
            }
        }
    }
}

/// 下载指定 Release 的 exe 资源到本地 `update` 目录并实时上报进度/速度。
///
/// 与 `run_download` 共享同一份内置 aria2 常驻进程（若尚未启动会自动启动），但为本次任务
/// 针对性地指定了 `updater::update_dir()` 目录（即 `%LOCALAPPDATA%\BingWallpaperLib\update`），
/// 避免混进用户配置的壁纸目录。下载中通过 `Rc<RefCell<Option<UpdateProgress>>>` 将最新
/// 已下/总字节/速度推送到“下载中”弹窗的 builder 闭包，同时 `cx.notify()` 触发重新渲染。
async fn run_update_download(
    aria2: &Rc<RefCell<Option<Rc<Aria2Manager>>>>,
    http: &Arc<dyn HttpClient>,
    release: &crate::updater::ReleaseInfo,
    progress: &Rc<RefCell<Option<UpdateProgress>>>,
    this: &WeakEntity<WallpaperLibrary>,
    cx: &mut AsyncApp,
) -> anyhow::Result<std::path::PathBuf> {
    let manager = ensure_aria2(aria2, http).await?;
    let dir = crate::updater::update_dir()?;
    let filename = release.asset_name.clone();
    let mut download_urls = vec![release.download_url.clone()];
    if let Some(fallback) = &release.fallback_download_url {
        download_urls.push(fallback.clone());
    }

    let mut last_error = None;
    for (index, url) in download_urls.iter().enumerate() {
        let _ = std::fs::remove_file(dir.join(&filename));
        let _ = std::fs::remove_file(dir.join(format!("{filename}.aria2")));
        *progress.borrow_mut() = Some(UpdateProgress::default());
        let _ = this.update(cx, |this, cx| {
            if index == 0 {
                this.set_status(format!("正在下载新版本 v{} ...", release.version), cx);
            } else {
                this.set_status("镜像下载失败，正在尝试 GitHub 官方地址...", cx);
            }
            cx.notify();
        });

        let gid = match manager.add_uri_to_dir(url, &dir, &filename).await {
            Ok(gid) => gid,
            Err(err) => {
                last_error = Some(err.to_string());
                continue;
            }
        };

        loop {
            let status = manager.tell_status(&gid).await?;
            let state = status
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string();

            let completed: u64 = status
                .get("completedLength")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let total: u64 = status
                .get("totalLength")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let speed: u64 = status
                .get("downloadSpeed")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            *progress.borrow_mut() = Some(UpdateProgress {
                completed,
                total,
                speed,
            });
            let _ = this.update(cx, |_this, cx| cx.notify());

            match state.as_str() {
                "complete" => return Ok(dir.join(&filename)),
                "error" => {
                    let msg = status
                        .get("errorMessage")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("未知错误")
                        .to_string();
                    last_error = Some(format!("aria2 下载失败: {msg}"));
                    break;
                }
                _ => {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(300))
                        .await;
                }
            }
        }
    }

    anyhow::bail!(
        "所有更新下载地址均失败: {}",
        last_error.unwrap_or_else(|| "未知错误".to_string())
    );
}

pub fn show_window_from_tray(window: &Window) {
    if let Some(hwnd) = hwnd_from_window(window) {
        unsafe {
            center_window(hwnd, 1200, 800);
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    } else {
        window.activate_window();
    }
}

unsafe fn center_window(hwnd: HWND, preferred_width: i32, preferred_height: i32) {
    let screen_width = GetSystemMetrics(SM_CXSCREEN);
    let screen_height = GetSystemMetrics(SM_CYSCREEN);
    if screen_width <= 0 || screen_height <= 0 {
        return;
    }

    let width = preferred_width.min(screen_width).max(200);
    let height = preferred_height.min(screen_height).max(200);
    let x = (screen_width - width) / 2;
    let y = (screen_height - height) / 2;

    let _ = SetWindowPos(
        hwnd,
        None,
        x,
        y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

pub fn hide_window_to_tray(window: &Window) {
    if let Some(hwnd) = hwnd_from_window(window) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    } else {
        window.minimize_window();
    }
}

fn hwnd_from_window(window: &Window) -> Option<HWND> {
    let handle = HasWindowHandle::window_handle(window).ok()?.as_raw();
    match handle {
        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut std::ffi::c_void)),
        _ => None,
    }
}

fn format_date_cn(date: NaiveDate) -> String {
    date.format("%Y年%m月%d日").to_string()
}

/// 人类可读的字节尺寸格式化：`1234567` → `"1.18 MiB"`。
///
/// 采用二进制前缀（KiB/MiB/GiB）而非十进制 KB/MB/GB，与大多数下载器的展示习惯一致。
fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// 人类可读的时长格式化（秒→中文“时分秒”形式）：`75` → `"1分12秒"`；`3720` → `"1时02分"`。
fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}时{:02}分", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}分{:02}秒", secs / 60, secs % 60)
    } else {
        format!("{secs}秒")
    }
}

impl Render for WallpaperLibrary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let years = self.years();
        let selected_key = self.selected_key.clone();
        let view_mode = self.view_mode;
        let sidebar_collapsed = self.sidebar_collapsed;
        let settings_panel_open = self.settings_panel_open;

        let mut sidebar_menu = SidebarMenu::new();
        for (year, months) in years {
            let mut year_item = SidebarMenuItem::new(SharedString::from(format!("{year} 年")))
                .icon(IconName::Calendar)
                .default_open(months.iter().any(|m| Some(&m.key) == selected_key.as_ref()))
                .click_to_toggle(true);

            let mut month_children = Vec::new();
            for month in months {
                let key = month.key.clone();
                let label = format!("{:02} 月 ({} 张)", month.month, month.entries.len());
                let is_active = view_mode == ViewMode::MonthDetail
                    && selected_key.as_deref() == Some(month.key.as_str());
                month_children.push(
                    SidebarMenuItem::new(SharedString::from(label))
                        .active(is_active)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select(key.clone(), cx);
                        })),
                );
            }
            year_item = year_item.children(month_children);
            sidebar_menu = sidebar_menu.child(year_item);
        }

        let home_item = SidebarMenuItem::new("主页")
            .icon(IconName::GalleryVerticalEnd)
            .active(view_mode == ViewMode::Home)
            .on_click(cx.listener(|this, _, _, cx| {
                this.view_mode = ViewMode::Home;
                cx.notify();
            }));

        let favorites_item = SidebarMenuItem::new("我的收藏")
            .icon(IconName::Heart)
            .active(view_mode == ViewMode::Favorites)
            .on_click(cx.listener(|this, _, _, cx| {
                this.view_mode = ViewMode::Favorites;
                cx.notify();
            }));

        let batch_download_item = SidebarMenuItem::new("批量下载")
            .active(view_mode == ViewMode::DownloadBatch)
            .on_click(cx.listener(|this, _, _, cx| {
                this.view_mode = ViewMode::DownloadBatch;
                cx.notify();
            }));

        let downloaded_item = SidebarMenuItem::new("已下载的壁纸")
            .active(view_mode == ViewMode::Downloaded)
            .on_click(cx.listener(|this, _, _, cx| {
                this.view_mode = ViewMode::Downloaded;
                cx.notify();
            }));

        let download_center_item = SidebarMenuItem::new("下载中心")
            .icon(IconName::FolderClosed)
            .default_open(matches!(
                view_mode,
                ViewMode::DownloadBatch | ViewMode::Downloaded
            ))
            .click_to_toggle(true)
            .children(vec![batch_download_item, downloaded_item]);

        let status = self.status.clone();

        let title_bar = TitleBar::new().child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .font_bold()
                .child(crate::paths::app_window_title()),
        );

        let main_row = h_flex()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(cx.theme().background)
            .child(
                Sidebar::new("main-sidebar")
                    .collapsible(SidebarCollapsible::Icon)
                    .collapsed(sidebar_collapsed)
                    .w(px(260.))
                    .header(
                        SidebarHeader::new().child(
                            h_flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .when(!sidebar_collapsed, |this| {
                                    this.child(
                                        v_flex()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .font_bold()
                                                    .child("Bing Daily Wallpaper (4K)"),
                                            )
                                            .child(div().text_xs().child("每日 4K 壁纸图库")),
                                    )
                                })
                                .child(
                                    SidebarToggleButton::new()
                                        .collapsed(sidebar_collapsed)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.sidebar_collapsed = !this.sidebar_collapsed;
                                            cx.notify();
                                        })),
                                ),
                        ),
                    )
                    .child(
                        SidebarGroup::new("导航").child(
                            SidebarMenu::new()
                                .child(home_item)
                                .child(favorites_item)
                                .child(download_center_item),
                        ),
                    )
                    .when(!sidebar_collapsed, |this| {
                        this.child(SidebarGroup::new("归档").child(sidebar_menu))
                    })
                    .footer(
                        v_flex().gap_2().p_2().w_full().child(
                            Button::new("open-settings")
                                .icon(IconName::Settings)
                                .ghost()
                                .small()
                                .tooltip("设置")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.settings_panel_open = !this.settings_panel_open;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(match view_mode {
                ViewMode::Home => self.render_home_view(status, cx).into_any_element(),
                ViewMode::Favorites => self.render_favorites_view(status, cx).into_any_element(),
                ViewMode::DownloadBatch => self
                    .render_batch_download_view(status, cx)
                    .into_any_element(),
                ViewMode::Downloaded => self.render_downloaded_view(status, cx).into_any_element(),
                ViewMode::MonthDetail => self
                    .render_month_view(selected_key, status, cx)
                    .into_any_element(),
            })
            .when(settings_panel_open, |this| {
                let view_for_overlay = cx.entity();
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| {
                            cx.stop_propagation();
                        })
                        .child(
                            Button::new("settings-panel-outside-close")
                                .label("")
                                .w_full()
                                .h_full()
                                .ghost()
                                .opacity(0.)
                                .on_click(move |_, _, cx| {
                                    view_for_overlay.update(cx, |this, cx| {
                                        this.settings_panel_open = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .left_3()
                        .bottom_3()
                        .child(self.render_settings_panel(cx)),
                )
            });

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .child(title_bar)
            .child(main_row)
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

impl WallpaperLibrary {
    fn render_settings_section_header(
        &self,
        id: &'static str,
        title: &'static str,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let opened = self.settings_section == Some(section);
        let view = cx.entity();

        h_flex()
            .id(id)
            .justify_between()
            .items_center()
            .w_full()
            .p_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(if opened {
                cx.theme().accent
            } else {
                cx.theme().border
            })
            .bg(cx.theme().background)
            .hover(|style| style.bg(cx.theme().accent.opacity(0.08)))
            .on_hover(move |hovered, _window, cx| {
                if *hovered {
                    view.update(cx, |this, cx| {
                        this.show_settings_section(section, cx);
                    });
                }
            })
            .child(div().text_sm().font_bold().child(title))
            .child(
                Icon::new(IconName::ChevronRight)
                    .size_4()
                    .text_color(if opened {
                        cx.theme().accent
                    } else {
                        cx.theme().muted_foreground
                    }),
            )
    }

    fn render_settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let input_for_field = self.settings_dir_input.clone();
        let input_for_open = self.settings_dir_input.clone();
        let input_for_choose = self.settings_dir_input.clone();
        let view = cx.entity();
        let theme_preference = self.settings.theme_preference;
        let wallpaper_target = self.settings.wallpaper_target.clone();
        let monitors = self.monitors.clone();
        let startup_enabled = self.settings.startup_enabled;
        let resident_enabled = self.settings.background_resident_enabled;
        let auto_enabled = self.settings.auto_wallpaper_enabled;
        let auto_source = self.settings.auto_wallpaper_source;
        let auto_hour = self.settings.auto_wallpaper_hour;
        let auto_minute = self.settings.auto_wallpaper_minute;

        let view_for_save = view.clone();
        let view_for_clear = view.clone();
        let view_for_check = view.clone();
        let view_for_system = view.clone();
        let view_for_light = view.clone();
        let view_for_dark = view.clone();
        let view_for_wallpaper_all = view.clone();
        let view_for_wallpaper_refresh = view.clone();

        let section = self
            .settings_section
            .unwrap_or(SettingsSection::DownloadDir);

        let detail: AnyElement = match section {
            SettingsSection::DownloadDir => v_flex()
                .gap_2()
                .child(div().text_sm().font_bold().child("壁纸下载保存路径"))
                .child(Input::new(&input_for_field))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("留空则使用默认目录；保存后若路径不存在会自动创建。"),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("settings-open-dir")
                                .label("打开目录")
                                .outline()
                                .small()
                                .on_click(move |_, _, cx| {
                                    let path = input_for_open.read(cx).value().to_string();
                                    open_in_explorer(&path);
                                }),
                        )
                        .child(
                            Button::new("settings-save-dir")
                                .label("选择并保存")
                                .primary()
                                .small()
                                .on_click(move |_, window, cx| {
                                    match crate::folder_picker::pick_folder() {
                                        Ok(Some(path)) => {
                                            let path_text = path.display().to_string();
                                            input_for_choose.update(cx, |input, cx| {
                                                input.set_value(path_text.clone(), window, cx);
                                            });
                                            view_for_save.update(cx, |this, cx| {
                                                this.apply_download_dir(path_text, cx);
                                            });
                                        }
                                        Ok(None) => {}
                                        Err(err) => {
                                            view_for_save.update(cx, |this, cx| {
                                                this.set_status(
                                                    format!("选择下载目录失败: {err}"),
                                                    cx,
                                                );
                                            });
                                        }
                                    }
                                }),
                        ),
                )
                .into_any_element(),
            SettingsSection::Appearance => v_flex()
                .gap_2()
                .child(div().text_sm().font_bold().child("外观模式"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("theme-system")
                                .label("跟随")
                                .small()
                                .when(theme_preference == ThemePreference::System, |this| {
                                    this.primary()
                                })
                                .when(theme_preference != ThemePreference::System, |this| {
                                    this.outline()
                                })
                                .on_click(move |_, window, cx| {
                                    view_for_system.update(cx, |this, cx| {
                                        this.set_theme_preference(
                                            ThemePreference::System,
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        )
                        .child(
                            Button::new("theme-light")
                                .label("白天")
                                .small()
                                .when(theme_preference == ThemePreference::Light, |this| {
                                    this.primary()
                                })
                                .when(theme_preference != ThemePreference::Light, |this| {
                                    this.outline()
                                })
                                .on_click(move |_, window, cx| {
                                    view_for_light.update(cx, |this, cx| {
                                        this.set_theme_preference(
                                            ThemePreference::Light,
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        )
                        .child(
                            Button::new("theme-dark")
                                .label("夜间")
                                .small()
                                .when(theme_preference == ThemePreference::Dark, |this| {
                                    this.primary()
                                })
                                .when(theme_preference != ThemePreference::Dark, |this| {
                                    this.outline()
                                })
                                .on_click(move |_, window, cx| {
                                    view_for_dark.update(cx, |this, cx| {
                                        this.set_theme_preference(
                                            ThemePreference::Dark,
                                            window,
                                            cx,
                                        );
                                    });
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("手动选择后不会再被系统主题变化覆盖。"),
                )
                .into_any_element(),
            SettingsSection::WallpaperTarget => v_flex()
                .gap_2()
                .child(div().text_sm().font_bold().child("多显示器壁纸"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("wallpaper-target-all")
                                .label("同步全部")
                                .small()
                                .when(wallpaper_target == WallpaperTarget::All, |this| {
                                    this.primary()
                                })
                                .when(wallpaper_target != WallpaperTarget::All, |this| {
                                    this.outline()
                                })
                                .on_click(move |_, _, cx| {
                                    view_for_wallpaper_all.update(cx, |this, cx| {
                                        this.set_wallpaper_target(WallpaperTarget::All, cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("wallpaper-refresh-monitors")
                                .label("刷新")
                                .outline()
                                .small()
                                .on_click(move |_, _, cx| {
                                    view_for_wallpaper_refresh.update(cx, |this, cx| {
                                        this.refresh_monitors(cx);
                                    });
                                }),
                        ),
                )
                .when(monitors.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("未检测到可单独设置的显示器；将使用同步全部显示器。"),
                    )
                })
                .when(!monitors.is_empty(), |this| {
                    this.child(div().flex().flex_wrap().gap_2().children(
                        monitors.into_iter().enumerate().map(|(index, monitor)| {
                            let selected =
                                wallpaper_target.monitor_id() == Some(monitor.id.as_str());
                            let view_for_monitor = view.clone();
                            let monitor_id = monitor.id.clone();
                            Button::new(SharedString::from(format!("wallpaper-monitor-{index}")))
                                .label(monitor.label)
                                .small()
                                .when(selected, |this| this.primary())
                                .when(!selected, |this| this.outline())
                                .on_click(move |_, _, cx| {
                                    let monitor_id = monitor_id.clone();
                                    view_for_monitor.update(cx, |this, cx| {
                                        this.set_wallpaper_target(
                                            WallpaperTarget::Monitor(monitor_id),
                                            cx,
                                        );
                                    });
                                })
                        }),
                    ))
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("选择后，所有“设为桌面壁纸”按钮都会按此目标生效。"),
                )
                .into_any_element(),
            SettingsSection::Automation => {
                let view_for_startup = view.clone();
                let view_for_resident = view.clone();
                let view_for_auto = view.clone();
                let view_for_now = view.clone();
                v_flex()
                    .gap_3()
                    .child(div().text_sm().font_bold().child("自动壁纸与后台"))
                    .child(
                        Checkbox::new("settings-startup-enabled")
                            .checked(startup_enabled)
                            .label("开机自启")
                            .on_click(move |_, _, cx| {
                                view_for_startup.update(cx, |this, cx| {
                                    this.toggle_startup_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        Checkbox::new("settings-resident-enabled")
                            .checked(resident_enabled)
                            .label("后台常驻 / 显示系统托盘图标")
                            .on_click(move |_, _, cx| {
                                view_for_resident.update(cx, |this, cx| {
                                    this.toggle_background_resident_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        Checkbox::new("settings-auto-wallpaper-enabled")
                            .checked(auto_enabled)
                            .label("每日自动更换壁纸")
                            .on_click(move |_, _, cx| {
                                view_for_auto.update(cx, |this, cx| {
                                    this.toggle_auto_wallpaper_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "当前计划：每天 {:02}:{:02}，来源：{}。程序运行或开机自启后会在后台检查执行。",
                                auto_hour,
                                auto_minute,
                                auto_source.label()
                            )),
                    )
                    .child(div().text_sm().font_bold().child("壁纸来源"))
                    .child(div().flex().flex_wrap().gap_2().children([
                        AutoWallpaperSource::Latest,
                        AutoWallpaperSource::RandomAll,
                        AutoWallpaperSource::RandomFavorites,
                    ].into_iter().map(|source| {
                        let view_for_source = view.clone();
                        Button::new(SharedString::from(format!("auto-source-{source:?}")))
                            .label(source.label())
                            .small()
                            .when(auto_source == source, |this| this.primary())
                            .when(auto_source != source, |this| this.outline())
                            .on_click(move |_, _, cx| {
                                view_for_source.update(cx, |this, cx| {
                                    this.set_auto_wallpaper_source(source, cx);
                                });
                            })
                    })))
                    .child(div().text_sm().font_bold().child("执行时间"))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .w(px(120.))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("小时"))
                                    .child(
                                        div()
                                            .id("auto-hour-scroll")
                                            .h(px(180.))
                                            .overflow_y_scroll()
                                            .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| cx.stop_propagation())
                                            .rounded(cx.theme().radius)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .p_1()
                                            .children((0u8..24).map(|hour| {
                                                let selected = auto_hour == hour;
                                                let view_for_time = view.clone();
                                                Button::new(SharedString::from(format!("auto-hour-{hour}")))
                                                    .label(format!("{:02}", hour))
                                                    .small()
                                                    .w_full()
                                                    .when(selected, |this| this.primary())
                                                    .when(!selected, |this| this.ghost())
                                                    .on_click(move |_, _, cx| {
                                                        view_for_time.update(cx, |this, cx| {
                                                            this.set_auto_wallpaper_time(hour, auto_minute, cx);
                                                        });
                                                    })
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .w(px(120.))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("分钟"))
                                    .child(
                                        div()
                                            .id("auto-minute-scroll")
                                            .h(px(180.))
                                            .overflow_y_scroll()
                                            .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| cx.stop_propagation())
                                            .rounded(cx.theme().radius)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .p_1()
                                            .children((0u8..60).map(|minute| {
                                                let selected = auto_minute == minute;
                                                let view_for_time = view.clone();
                                                Button::new(SharedString::from(format!("auto-minute-{minute}")))
                                                    .label(format!("{:02}", minute))
                                                    .small()
                                                    .w_full()
                                                    .when(selected, |this| this.primary())
                                                    .when(!selected, |this| this.ghost())
                                                    .on_click(move |_, _, cx| {
                                                        view_for_time.update(cx, |this, cx| {
                                                            this.set_auto_wallpaper_time(auto_hour, minute, cx);
                                                        });
                                                    })
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("当前选择"))
                                    .child(div().text_2xl().font_bold().child(format!("{:02}:{:02}", auto_hour, auto_minute)))
                                    .child(div().text_xs().text_color(cx.theme().muted_foreground).child("像闹钟一样分别选择小时和分钟")),
                            ),
                    )
                    .child(
                        Button::new("auto-wallpaper-now")
                            .label("立即按当前方案更换一次")
                            .outline()
                            .w_full()
                            .on_click(move |_, _, cx| {
                                view_for_now.update(cx, |this, cx| {
                                    this.trigger_auto_wallpaper_now(cx);
                                });
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("提示：选择“每日最新壁纸”时，会自动下载当天最新图片并设为桌面壁纸；选择随机收藏前，请先添加收藏。"),
                    )
                    .into_any_element()
            }
            SettingsSection::Maintenance => v_flex()
                .gap_2()
                .child(div().text_sm().font_bold().child("维护"))
                .child(
                    Button::new("settings-clear-cache")
                        .label("清空壁纸缓存")
                        .outline()
                        .w_full()
                        .on_click(move |_, _, cx| {
                            view_for_clear.update(cx, |this, cx| {
                                this.clear_cache(cx);
                            });
                        }),
                )
                .child(
                    Button::new("settings-check-update")
                        .label("检查更新")
                        .outline()
                        .w_full()
                        .on_click(move |_, window, cx| {
                            view_for_check.update(cx, |this, cx| {
                                this.check_for_updates(true, window, cx);
                            });
                        }),
                )
                .child(
                    Button::new("settings-about")
                        .label("关于软件")
                        .outline()
                        .w_full()
                        .on_click(|_, window, cx| {
                            open_about_dialog(window, cx);
                        }),
                )
                .into_any_element(),
        };

        h_flex()
            .w(px(760.))
            .h(px(560.))
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| {
                cx.stop_propagation();
            })
            .child(
                v_flex()
                    .id("settings-section-list")
                    .w(px(200.))
                    .h_full()
                    .flex_shrink_0()
                    .gap_2()
                    .p_3()
                    .overflow_y_scroll()
                    .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| {
                        cx.stop_propagation();
                    })
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(div().font_bold().child("设置"))
                    .child(self.render_settings_section_header(
                        "settings-section-download",
                        "下载路径",
                        SettingsSection::DownloadDir,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-appearance",
                        "外观模式",
                        SettingsSection::Appearance,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-wallpaper-target",
                        "多显示器壁纸",
                        SettingsSection::WallpaperTarget,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-automation",
                        "自动壁纸",
                        SettingsSection::Automation,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-maintenance",
                        "维护",
                        SettingsSection::Maintenance,
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .id("settings-detail-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap_2()
                    .p_3()
                    .overflow_y_scroll()
                    .on_scroll_wheel(|_event: &ScrollWheelEvent, _window, cx| {
                        cx.stop_propagation();
                    })
                    .child(detail),
            )
    }

    fn render_home_view(&self, status: SharedString, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.flat_entries.len();
        let show_count = self.home_loaded_count.min(total);
        let entries: Vec<WallpaperEntry> = self.flat_entries[..show_count].to_vec();

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .child(format!("首页 · 最近壁纸（{show_count}/{total}）")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
            .child(
                div()
                    .id("home-scroll-wrap")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("home-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.home_scroll_handle)
                            .on_scroll_wheel(cx.listener(
                                |this, _event: &ScrollWheelEvent, _window, cx| {
                                    this.maybe_load_more_home(cx);
                                },
                            ))
                            .child(
                                div().flex().flex_wrap().gap_4().pr_2().children(
                                    entries
                                        .into_iter()
                                        .map(|entry| self.render_home_card(entry, cx)),
                                ),
                            ),
                    )
                    .vertical_scrollbar(&self.home_scroll_handle)
                    .child(
                        div().absolute().bottom_6().right_6().child(
                            Button::new("home-back-to-top")
                                .icon(IconName::ArrowUp)
                                .ghost()
                                .opacity(0.6)
                                .tooltip("回到顶部")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.home_scroll_handle.set_offset(point(px(0.), px(0.)));
                                    cx.notify();
                                })),
                        ),
                    ),
            )
    }

    fn render_favorites_view(
        &self,
        status: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entries = self.favorite_entries();
        let count = entries.len();

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .child(format!("我的收藏（{count} 张）")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
            .child(
                div()
                    .id("favorites-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(if entries.is_empty() {
                        v_flex()
                            .items_center()
                            .justify_center()
                            .h_full()
                            .gap_2()
                            .child(div().text_lg().font_bold().child("还没有收藏壁纸"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("在首页或归档中点击 ❤ 即可收藏喜欢的壁纸"),
                            )
                            .into_any_element()
                    } else {
                        v_flex()
                            .gap_3()
                            .children(
                                entries
                                    .into_iter()
                                    .map(|entry| self.render_entry_card(entry, cx)),
                            )
                            .into_any_element()
                    }),
            )
    }

    fn render_batch_download_view(
        &self,
        status: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let batch_range_picker = self.batch_range_picker.clone();
        let batch_range_picker_for_read = self.batch_range_picker.clone();
        let view = cx.entity();
        let view_for_all = view.clone();
        let view_for_month = view.clone();
        let view_for_favorites = view.clone();
        let view_for_range = view.clone();

        let all_entries = self.flat_entries.clone();
        let month_entries = self
            .selected_group()
            .map(|group| group.entries.clone())
            .unwrap_or_default();
        let favorite_entries = self.favorite_entries();
        let batch_progress = self.batch_progress;
        let date_limits = self
            .batch_date_limits
            .lock()
            .ok()
            .and_then(|limits| *limits);
        let date_range_hint: SharedString = match date_limits {
            Some((earliest, latest)) => format!(
                "可选范围：{} 至 {}；超出范围的日期会自动禁用。",
                format_date_cn(earliest),
                format_date_cn(latest)
            )
            .into(),
            None => "壁纸列表加载完成后才能选择日期范围。".into(),
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_bold().text_lg().child("下载中心 · 批量下载"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_4()
                    .p_4()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(div().text_sm().font_bold().child("快速下载"))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("batch-center-all")
                                            .label("全部历史")
                                            .outline()
                                            .disabled(batch_progress.is_some())
                                            .on_click(move |_, _, cx| {
                                                let entries = all_entries.clone();
                                                view_for_all.update(cx, |this, cx| {
                                                    this.start_batch_download(
                                                        "全部历史壁纸",
                                                        entries,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("batch-center-month")
                                            .label("当前月份")
                                            .outline()
                                            .disabled(batch_progress.is_some())
                                            .on_click(move |_, _, cx| {
                                                let entries = month_entries.clone();
                                                view_for_month.update(cx, |this, cx| {
                                                    this.start_batch_download(
                                                        "当前月份",
                                                        entries,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("batch-center-favorites")
                                            .label("收藏")
                                            .outline()
                                            .disabled(batch_progress.is_some())
                                            .on_click(move |_, _, cx| {
                                                let entries = favorite_entries.clone();
                                                view_for_favorites.update(cx, |this, cx| {
                                                    this.start_batch_download(
                                                        "我的收藏",
                                                        entries,
                                                        cx,
                                                    );
                                                });
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(div().text_sm().font_bold().child("按日期范围下载"))
                            .child(
                                DatePicker::new(&batch_range_picker)
                                    .placeholder("请选择日期范围")
                                    .cleanable(true)
                                    .disabled(date_limits.is_none())
                                    .w(px(360.)),
                            )
                            .child(
                                Button::new("batch-center-date-range")
                                    .label("下载日期范围")
                                    .outline()
                                    .disabled(batch_progress.is_some() || date_limits.is_none())
                                    .on_click(move |_, _, cx| {
                                        let date = batch_range_picker_for_read.read(cx).date();
                                        let start = date.start();
                                        let end = date.end();
                                        view_for_range.update(cx, |this, cx| {
                                            this.start_date_range_batch_download(start, end, cx);
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(date_range_hint),
                            ),
                    )
                    .when_some(batch_progress, |this, progress| {
                        this.child(
                            v_flex()
                                .gap_1()
                                .child(div().text_sm().font_bold().child("下载进度"))
                                .child(Progress::new("batch-center-progress").value(
                                    if progress.total == 0 {
                                        0.0
                                    } else {
                                        progress.completed as f32 / progress.total as f32 * 100.0
                                    },
                                ))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!(
                                            "{}/{}，跳过 {}，失败 {}",
                                            progress.completed,
                                            progress.total,
                                            progress.skipped,
                                            progress.failed
                                        )),
                                ),
                        )
                    }),
            )
    }

    /// 扫描当前生效的壁纸下载目录，返回已下载的图片文件列表（按文件名倒序，
    /// 文件名以日期开头，因此倒序即最新在前）。
    fn downloaded_files(&self) -> Vec<std::path::PathBuf> {
        let dir = match crate::paths::wallpapers_dir() {
            Ok(dir) => dir,
            Err(_) => return Vec::new(),
        };
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| {
                                ext.eq_ignore_ascii_case("jpg")
                                    || ext.eq_ignore_ascii_case("jpeg")
                                    || ext.eq_ignore_ascii_case("png")
                                    || ext.eq_ignore_ascii_case("webp")
                            })
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        files
    }

    fn render_downloaded_view(
        &self,
        status: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let files = self.downloaded_files();
        let count = files.len();
        let all_selected = count > 0 && files.iter().all(|p| self.downloaded_selected.contains(p));
        let selected_count = self.downloaded_selected.len();
        let dir_display = crate::paths::wallpapers_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default();

        let view_for_select_all = cx.entity();
        let files_for_select_all = files.clone();
        let view_for_delete_selected = cx.entity();

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .child(format!("下载中心 · 已下载的壁纸（{count} 张）")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(status),
                            )
                            .child(
                                Button::new("downloaded-select-all")
                                    .label(if all_selected {
                                        "取消全选"
                                    } else {
                                        "全选"
                                    })
                                    .outline()
                                    .small()
                                    .disabled(count == 0)
                                    .on_click(move |_, _, cx| {
                                        let files = files_for_select_all.clone();
                                        view_for_select_all.update(cx, |this, cx| {
                                            if all_selected {
                                                this.downloaded_selected.clear();
                                            } else {
                                                this.downloaded_selected =
                                                    files.into_iter().collect();
                                            }
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(
                                Button::new("downloaded-delete-selected")
                                    .label(format!("删除选中 ({selected_count})"))
                                    .danger()
                                    .small()
                                    .disabled(selected_count == 0)
                                    .on_click(move |_, _, cx| {
                                        view_for_delete_selected.update(cx, |this, cx| {
                                            this.delete_selected_downloaded(cx);
                                        });
                                    }),
                            )
                            .child(
                                Button::new("downloaded-refresh")
                                    .label("刷新")
                                    .outline()
                                    .small()
                                    .on_click(cx.listener(|_, _, _, cx| cx.notify())),
                            )
                            .child(
                                Button::new("downloaded-open-dir")
                                    .label("打开下载目录")
                                    .outline()
                                    .small()
                                    .on_click(move |_, _, _cx| {
                                        open_in_explorer(&dir_display);
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("downloaded-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(if files.is_empty() {
                        v_flex()
                            .items_center()
                            .justify_center()
                            .h_full()
                            .gap_2()
                            .child(div().text_lg().font_bold().child("还没有已下载的壁纸"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("在首页、归档或批量下载中下载壁纸后会显示在这里"),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_4()
                            .children(
                                files
                                    .into_iter()
                                    .map(|path| self.render_downloaded_card(path, cx)),
                            )
                            .into_any_element()
                    }),
            )
    }

    /// 已下载壁纸画廊中的单张卡片：点击图片本身预览（弹窗内可设为桌面壁纸/删除），
    /// 左上角勾选框用于批量选择，鼠标悬停时图片底部浮现“设为桌面壁纸”/删除按钮。
    fn render_downloaded_card(
        &self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let group_name: SharedString = format!("downloaded-card-{name}").into();
        let path_for_preview = path.clone();
        let path_for_wallpaper = path.clone();
        let path_for_delete = path.clone();
        let path_for_checkbox = path.clone();
        let is_selected = self.downloaded_selected.contains(&path);

        v_flex()
            .group(group_name.clone())
            .w(px(220.))
            .gap_1()
            .child(
                div()
                    .relative()
                    .w(px(220.))
                    .h(px(124.))
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(img(path.clone()).w(px(220.)).h(px(124.)))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(
                                Button::new(SharedString::from(format!("downloaded-image-{name}")))
                                    .label("")
                                    .w_full()
                                    .h_full()
                                    .ghost()
                                    .opacity(0.)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.open_local_preview_dialog(
                                            path_for_preview.clone(),
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_1()
                            .left_1()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                Checkbox::new(SharedString::from(format!(
                                    "downloaded-check-{name}"
                                )))
                                .checked(is_selected)
                                .on_click({
                                    let view = cx.entity();
                                    move |_, _, cx| {
                                        let path = path_for_checkbox.clone();
                                        view.update(cx, |this, cx| {
                                            if this.downloaded_selected.contains(&path) {
                                                this.downloaded_selected.remove(&path);
                                            } else {
                                                this.downloaded_selected.insert(path);
                                            }
                                            cx.notify();
                                        });
                                    }
                                }),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_1()
                            .left_1()
                            .right_1()
                            .opacity(0.)
                            .group_hover(group_name.clone(), |style| style.opacity(1.))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "downloaded-set-{name}"
                                        )))
                                        .label("设为桌面壁纸")
                                        .primary()
                                        .small()
                                        .flex_1()
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.set_local_file_as_wallpaper(
                                                    path_for_wallpaper.clone(),
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "downloaded-delete-{name}"
                                        )))
                                        .icon(IconName::Delete)
                                        .danger()
                                        .ghost()
                                        .small()
                                        .tooltip("删除")
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.delete_downloaded_file(
                                                    path_for_delete.clone(),
                                                    cx,
                                                );
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(name),
            )
    }

    /// 打开本地已下载壁纸的预览对话框：图片已存在于本地，因此无需“下载”按钮，
    /// 只提供“删除”与“设为桌面壁纸”。同样遵守 `open_preview_dialog` 的重入规避约定：
    /// 不在对话框 builder 闭包内对 `cx.entity()` 调用 `.read()`。
    fn open_local_preview_dialog(
        &self,
        path: std::path::PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.entity();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let path_for_title = path.clone();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_for_delete = view.clone();
            let view_for_wall = view.clone();
            let path_for_delete = path.clone();
            let path_for_wall = path.clone();

            dialog
                .title(
                    path_for_title
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
                .w(px(860.))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(img(path.clone()).w(px(800.)).h(px(450.)).rounded(px(6.)))
                        .child(div().text_sm().child(name.clone())),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("local-preview-delete")
                                .label("删除")
                                .danger()
                                .on_click(move |_, window, cx| {
                                    let path = path_for_delete.clone();
                                    view_for_delete.update(cx, |this, cx| {
                                        this.delete_downloaded_file(path, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("local-preview-set-wallpaper")
                                .label("设为桌面壁纸")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let path = path_for_wall.clone();
                                    view_for_wall.update(cx, |this, cx| {
                                        this.set_local_file_as_wallpaper(path, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    fn render_month_view(
        &self,
        selected_key: Option<String>,
        status: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let _ = selected_key;
        let content = self.selected_group().cloned();

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_bold().text_lg().child(match &content {
                        Some(group) => {
                            format!("{}年{:02}月", group.year, group.month)
                        }
                        None => "请选择左侧月份".to_string(),
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
            .child(
                div()
                    .id("wallpaper-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        v_flex().gap_3().children(
                            content
                                .map(|group| group.entries.clone())
                                .unwrap_or_default()
                                .into_iter()
                                .map(|entry| self.render_entry_card(entry, cx)),
                        ),
                    ),
            )
    }

    /// 首页网格中的单张壁纸卡片：点击图片本身预览；鼠标悬停时图片底部浮现
    /// “设为桌面壁纸”按钮（纯 CSS 式的 group-hover 实现，不依赖额外应用状态）。
    fn render_home_card(&self, entry: WallpaperEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let group_name: SharedString = format!("home-card-{}", entry.date).into();
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        let entry_for_preview = entry.clone();
        let entry_for_wallpaper = entry.clone();
        let favorite_date = entry.date;
        let is_favorite = self.favorites.contains(&entry.date);
        let progress = self.progress.get(&entry.date).copied();

        v_flex()
            .group(group_name.clone())
            .w(px(260.))
            .gap_2()
            .child(
                div()
                    .relative()
                    .w(px(260.))
                    .h(px(146.))
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(img(entry.thumbnail_url()).w(px(260.)).h(px(146.)))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .child(
                                Button::new(SharedString::from(format!(
                                    "home-image-{}",
                                    entry.date
                                )))
                                .label("")
                                .w_full()
                                .h_full()
                                .ghost()
                                .opacity(0.)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.open_preview_dialog(
                                            entry_for_preview.clone(),
                                            window,
                                            cx,
                                        );
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_2()
                            .left_2()
                            .right_2()
                            .opacity(0.)
                            .group_hover(group_name.clone(), |style| style.opacity(1.))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "home-set-{}",
                                            entry.date
                                        )))
                                        .label("设为桌面壁纸")
                                        .primary()
                                        .small()
                                        .flex_1()
                                        .disabled(progress.is_some())
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.set_as_wallpaper(
                                                    entry_for_wallpaper.clone(),
                                                    cx,
                                                );
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "home-fav-{}",
                                            entry.date
                                        )))
                                        .icon(
                                            Icon::empty()
                                                .path(if is_favorite {
                                                    "icons/heart-filled.svg"
                                                } else {
                                                    "icons/heart-outline.svg"
                                                })
                                                .text_color(if is_favorite {
                                                    hsla(0., 0.85, 0.55, 1.)
                                                } else {
                                                    cx.theme().muted_foreground
                                                })
                                                .size_6(),
                                        )
                                        .ghost()
                                        .small()
                                        .tooltip(if is_favorite {
                                            "取消收藏"
                                        } else {
                                            "收藏"
                                        })
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.toggle_favorite(favorite_date, cx);
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_sm().font_bold().child(date_str))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(entry.title.clone()),
                    ),
            )
    }

    fn render_entry_card(&self, entry: WallpaperEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let entry_for_download = entry.clone();
        let entry_for_preview = entry.clone();
        let entry_for_wallpaper = entry.clone();
        let favorite_date = entry.date;
        let is_favorite = self.favorites.contains(&entry.date);
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        let progress = self.progress.get(&entry.date).copied();

        h_flex()
            .gap_3()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .child(
                img(entry.thumbnail_url())
                    .w(px(220.))
                    .h(px(124.))
                    .rounded(cx.theme().radius),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(div().font_bold().child(date_str))
                    .child(div().text_sm().child(entry.title.clone()))
                    .child(
                        h_flex()
                            .gap_2()
                            .mt_2()
                            .child(
                                Button::new(SharedString::from(format!("dl-{}", entry.date)))
                                    .label("下载")
                                    .disabled(progress.is_some())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.start_download(entry_for_download.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("preview-{}", entry.date)))
                                    .label("预览图片")
                                    .outline()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_preview_dialog(
                                            entry_for_preview.clone(),
                                            window,
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("set-{}", entry.date)))
                                    .label("设为桌面壁纸")
                                    .primary()
                                    .disabled(progress.is_some())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_as_wallpaper(entry_for_wallpaper.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("fav-{}", entry.date)))
                                    .icon(
                                        Icon::empty()
                                            .path(if is_favorite {
                                                "icons/heart-filled.svg"
                                            } else {
                                                "icons/heart-outline.svg"
                                            })
                                            .text_color(if is_favorite {
                                                hsla(0., 0.85, 0.55, 1.)
                                            } else {
                                                cx.theme().muted_foreground
                                            })
                                            .size_6(),
                                    )
                                    .ghost()
                                    .tooltip(if is_favorite {
                                        "取消收藏"
                                    } else {
                                        "收藏"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_favorite(favorite_date, cx);
                                    })),
                            ),
                    )
                    .when_some(progress, |this, percent| {
                        this.child(
                            v_flex()
                                .gap_1()
                                .mt_1()
                                .child(
                                    Progress::new(SharedString::from(format!(
                                        "progress-{}",
                                        entry.date
                                    )))
                                    .value(percent),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("{percent:.0}%")),
                                ),
                        )
                    }),
            )
    }
}
