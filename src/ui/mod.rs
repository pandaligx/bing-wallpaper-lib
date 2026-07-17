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
use crate::fetcher;
use crate::i18n::LanguagePreference;
use crate::model::{group_by_month, MonthGroup, WallpaperEntry};
use crate::settings::{
    AppSettings, AutoWallpaperSource, DownloadResolution, ThemePreference, WallpaperTarget,
};
use crate::wallpaper_setter;
use crate::window_sizing;
use chrono::{Datelike, Local, NaiveDate, Timelike};
use futures::{future::Shared, FutureExt};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::alert::Alert;
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
use gpui_component::{
    button::Button, theme::ThemeMode, v_virtual_list, Root, Theme, VirtualListScrollHandle,
};
use http_client::HttpClient;
use rand::seq::SliceRandom;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    SetForegroundWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
    SW_SHOWNORMAL,
};

/// 软件版权/作者署名，展示于“关于”信息中。
const COPYRIGHT: &str = "© 2023-2026 小南瓜";

/// 首页壁纸卡片固定宽度。
const HOME_GRID_CARD_WIDTH: f32 = 260.0;

/// 首页壁纸卡片列间距（对应 `.gap_4()`）。
const HOME_GRID_GAP: f32 = 16.0;

/// 首页虚拟网格中单行的固定高度，用于 `VirtualList` 计算可见范围。
const HOME_GRID_ROW_HEIGHT: f32 = 245.0;

/// 展开侧边栏的近似宽度，用于按窗口宽度计算首页虚拟网格列数。
const SIDEBAR_EXPANDED_WIDTH: f32 = 260.0;

/// 折叠侧边栏的近似宽度。
const SIDEBAR_COLLAPSED_WIDTH: f32 = 56.0;

/// 远程缩略图最多保留的数量。超出后会释放最久未使用的图片纹理，避免滚动完整历史后
/// 进程内存随图片数量无限增长。
const THUMBNAIL_IMAGE_CACHE_ITEMS: usize = 120;

/// 本地画廊只保留少量已经缩小的图片纹理，避免滚动后持续累积。
const DOWNLOADED_THUMBNAIL_CACHE_ITEMS: usize = 24;

/// 已下载壁纸画廊的固定卡片布局参数。
const DOWNLOADED_GRID_CARD_WIDTH: f32 = 220.0;
const DOWNLOADED_GRID_GAP: f32 = 16.0;
const DOWNLOADED_GRID_ROW_HEIGHT: f32 = 178.0;

fn image_frame(
    source: impl Into<ImageSource>,
    width: f32,
    height: f32,
    language: LanguagePreference,
    cx: &mut App,
) -> impl IntoElement {
    div()
        .relative()
        .w(px(width))
        .h(px(height))
        .rounded(cx.theme().radius)
        .overflow_hidden()
        .bg(cx.theme().muted)
        .child(
            v_flex()
                .absolute()
                .inset_0()
                .items_center()
                .justify_center()
                .gap_1()
                .text_color(cx.theme().muted_foreground)
                .child(Icon::new(IconName::Frame).size_6())
                .child(div().text_xs().child(language.t("Image loading..."))),
        )
        .child(
            img(source)
                .absolute()
                .inset_0()
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Cover),
        )
}

fn preview_dialog_dimensions(window: &Window) -> (f32, f32, f32) {
    let viewport = window.viewport_size();
    let viewport_width = viewport.width.as_f32().max(320.0);
    let viewport_height = viewport.height.as_f32().max(320.0);

    let max_dialog_width = (viewport_width - 48.0).clamp(320.0, 860.0);
    let max_image_width = (max_dialog_width - 60.0).clamp(260.0, 800.0);
    let max_image_height = (viewport_height - 220.0).clamp(180.0, 450.0);
    let image_width = max_image_width.min(max_image_height * 16.0 / 9.0);
    let image_height = image_width * 9.0 / 16.0;
    let dialog_width = (image_width + 60.0).clamp(320.0, max_dialog_width);

    (dialog_width, image_width, image_height)
}

type ImageLoadingTask = Shared<Task<Result<Arc<RenderImage>, ImageCacheError>>>;

struct BoundedImageCache {
    max_items: usize,
    usages: Vec<u64>,
    cache: HashMap<u64, ImageCacheItem>,
}

impl BoundedImageCache {
    fn new(max_items: usize, cx: &mut Context<Self>) -> Self {
        cx.on_release(|cache, cx| {
            for (_, mut item) in std::mem::take(&mut cache.cache) {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, None);
                }
            }
        })
        .detach();

        Self {
            max_items,
            usages: Vec::with_capacity(max_items),
            cache: HashMap::with_capacity(max_items),
        }
    }

    fn clear(&mut self, window: &mut Window, cx: &mut App) {
        for (_, mut item) in std::mem::take(&mut self.cache) {
            if let Some(Ok(image)) = item.get() {
                cx.drop_image(image, Some(window));
            }
        }
        self.usages.clear();
    }
}

impl ImageCache for BoundedImageCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        if self.max_items == 0 {
            return None;
        }

        let hash = hash(resource);
        if let Some(item) = self.cache.get_mut(&hash) {
            if let Some(current_ix) = self.usages.iter().position(|item| *item == hash) {
                self.usages.remove(current_ix);
            }
            self.usages.insert(0, hash);
            return item.get();
        }

        let fut = AssetLogger::<ImageAssetLoader>::load(resource.clone(), cx);
        let task: ImageLoadingTask = cx.background_executor().spawn(fut).shared();

        while self.usages.len() >= self.max_items {
            let Some(oldest) = self.usages.pop() else {
                break;
            };
            if let Some(mut item) = self.cache.remove(&oldest) {
                if let Some(Ok(image)) = item.get() {
                    cx.drop_image(image, Some(window));
                }
            }
        }

        self.cache
            .insert(hash, ImageCacheItem::Loading(task.clone()));
        self.usages.insert(0, hash);

        let entity = window.current_view();
        window
            .spawn(cx, async move |cx| {
                _ = task.await;
                cx.on_next_frame(move |_, cx| {
                    cx.notify(entity);
                });
            })
            .detach();

        None
    }
}

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
    aria2_start_lock: Rc<smol::lock::Mutex<()>>,
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
    /// 首页虚拟网格滚动状态句柄，用于右侧滚动条和回到顶部。
    home_scroll_handle: VirtualListScrollHandle,
    /// 远程缩略图 LRU 缓存，限制主页/月份列表滚动时保留的图片数量。
    thumbnail_cache: Entity<BoundedImageCache>,
    /// 已下载壁纸虚拟网格的滚动状态句柄。
    downloaded_scroll_handle: VirtualListScrollHandle,
    /// 本地画廊缩略图的独立小容量 LRU，只缓存实际缩小后的图片。
    downloaded_thumbnail_cache: Entity<BoundedImageCache>,
    /// 等待后台串行生成的本地缩略图，避免同时解码多张 4K 原图。
    downloaded_thumbnail_queue: VecDeque<(
        std::path::PathBuf,
        crate::local_thumbnails::SourceFingerprint,
        u64,
    )>,
    downloaded_thumbnail_pending: HashSet<crate::local_thumbnails::SourceFingerprint>,
    downloaded_thumbnail_failures: HashSet<crate::local_thumbnails::SourceFingerprint>,
    downloaded_thumbnail_epoch: u64,
    downloaded_thumbnail_worker_running: bool,
    /// 侧边导航栏是否处于折叠（仅图标）状态。
    sidebar_collapsed: bool,
    /// 设置浮层是否展开（左下角，类似抖音侧边栏设置菜单）。
    settings_panel_open: bool,
    /// 语言选择浮层是否展开。
    language_panel_open: bool,
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
    /// 每日自动壁纸是否正在执行，避免 5 秒轮询重复提交同一天任务。
    auto_wallpaper_running: bool,
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
        let language = settings.language;
        let settings_dir_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("{}: {default_dir_display}", language.t("Default")))
                .default_value(initial_dir_text)
        });
        let batch_date_limits = Arc::new(Mutex::new(None::<(NaiveDate, NaiveDate)>));
        let limits_for_picker = batch_date_limits.clone();
        let batch_range_picker = cx.new(|cx| {
            DatePickerState::range(window, cx)
                .date_format("%Y-%m-%d")
                .disabled_matcher(move |date: &NaiveDate| {
                    limits_for_picker
                        .lock()
                        .ok()
                        .and_then(|limits| *limits)
                        .is_some_and(|(earliest, latest)| *date < earliest || *date > latest)
                })
        });
        let monitors = wallpaper_setter::list_monitors().unwrap_or_default();
        let initial_status = settings.language.t("Loading wallpaper list...");

        Self {
            groups: Vec::new(),
            flat_entries: Vec::new(),
            view_mode: ViewMode::Home,
            selected_key: None,
            status: initial_status.into(),
            aria2: Rc::new(RefCell::new(None)),
            aria2_start_lock: Rc::new(smol::lock::Mutex::new(())),
            http: cx.http_client(),
            progress: HashMap::new(),
            favorites: crate::favorites::load(),
            batch_progress: None,
            monitors,
            update_progress: Rc::new(RefCell::new(None)),
            home_scroll_handle: VirtualListScrollHandle::new(),
            thumbnail_cache: cx.new(|cx| BoundedImageCache::new(THUMBNAIL_IMAGE_CACHE_ITEMS, cx)),
            downloaded_scroll_handle: VirtualListScrollHandle::new(),
            downloaded_thumbnail_cache: cx
                .new(|cx| BoundedImageCache::new(DOWNLOADED_THUMBNAIL_CACHE_ITEMS, cx)),
            downloaded_thumbnail_queue: VecDeque::new(),
            downloaded_thumbnail_pending: HashSet::new(),
            downloaded_thumbnail_failures: HashSet::new(),
            downloaded_thumbnail_epoch: 0,
            downloaded_thumbnail_worker_running: false,
            sidebar_collapsed: false,
            settings_panel_open: false,
            language_panel_open: false,
            settings_section: None,
            settings,
            settings_dir_input,
            batch_range_picker,
            batch_date_limits,
            downloaded_selected: HashSet::new(),
            auto_wallpaper_running: false,
        }
    }

    /// 导出内部持有的 aria2 管理器共享句柄，供应用退出时优雅关闭使用（见 `main.rs`）。
    pub fn aria2_handle(&self) -> Rc<RefCell<Option<Rc<Aria2Manager>>>> {
        self.aria2.clone()
    }

    fn cancel_downloaded_thumbnail_work(&mut self) {
        self.downloaded_thumbnail_epoch = self.downloaded_thumbnail_epoch.wrapping_add(1);
        self.downloaded_thumbnail_queue.clear();
        self.downloaded_thumbnail_pending.clear();
    }

    fn clear_downloaded_thumbnail_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_downloaded_thumbnail_work();
        self.downloaded_thumbnail_cache.update(cx, |cache, cx| {
            cache.clear(window, cx);
        });
    }

    fn clear_thumbnail_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.thumbnail_cache.update(cx, |cache, cx| {
            cache.clear(window, cx);
        });
        self.clear_downloaded_thumbnail_cache(window, cx);
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

    fn refresh_wallpaper_list(&mut self, cx: &mut Context<Self>) {
        self.set_status("正在重新获取壁纸列表...", cx);
        let http = self.http.clone();
        cx.spawn(async move |this, cx| {
            match fetcher::fetch_all(http).await {
                Ok(entries) => {
                    let is_new = fetcher::load_cache()
                        .ok()
                        .flatten()
                        .map(|cached| fetcher::has_new_entry(&cached, &entries))
                        .unwrap_or(true);
                    let _ = fetcher::save_cache(&entries);
                    let _ = this.update(cx, |this, cx| {
                        this.set_entries(entries, cx);
                        if is_new {
                            this.set_status("已重新获取壁纸列表，并检测到新壁纸", cx);
                        } else {
                            this.set_status("已重新获取壁纸列表，当前已是最新", cx);
                        }
                    });
                }
                Err(err) => {
                    let bundled = fetcher::bundled_entries();
                    let _ = this.update(cx, |this, cx| {
                        if bundled.is_empty() {
                            this.set_status(format!("获取壁纸列表失败: {err}"), cx);
                        } else {
                            let count = bundled.len();
                            this.set_entries(bundled, cx);
                            this.set_status(
                                format!(
                                    "远程壁纸列表获取失败，已使用内置壁纸列表（{count} 张）。请稍后重试: {err}"
                                ),
                                cx,
                            );
                        }
                    });
                }
            }
        })
        .detach();
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

    fn set_view_mode(&mut self, view_mode: ViewMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.view_mode == ViewMode::Downloaded && view_mode != ViewMode::Downloaded {
            self.clear_downloaded_thumbnail_cache(window, cx);
        }
        if view_mode == ViewMode::Downloaded {
            let files = self.downloaded_files();
            if let Err(err) = crate::local_thumbnails::prune(&files) {
                log::warn!("清理本地壁纸缩略图失败: {err:#}");
            }
        }
        self.view_mode = view_mode;
        cx.notify();
    }

    fn select(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_key = Some(key);
        self.set_view_mode(ViewMode::MonthDetail, window, cx);
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
        let should_check_now = if self.settings.auto_wallpaper_enabled {
            self.prepare_auto_wallpaper_after_manual_setting_change()
        } else {
            self.auto_wallpaper_running = false;
            false
        };
        let _ = self.settings.save();
        self.set_status(
            if self.settings.auto_wallpaper_enabled {
                "已开启每日自动壁纸"
            } else {
                "已关闭每日自动壁纸"
            },
            cx,
        );
        if should_check_now {
            self.check_scheduled_wallpaper(cx);
        }
    }

    pub fn toggle_auto_wallpaper_exit_after_done(&mut self, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_exit_after_done =
            !self.settings.auto_wallpaper_exit_after_done;
        let _ = self.settings.save();
        self.set_status(
            if self.settings.auto_wallpaper_exit_after_done {
                "已开启自动壁纸完成后退出程序"
            } else {
                "已关闭自动壁纸完成后退出程序"
            },
            cx,
        );
    }

    fn prepare_auto_wallpaper_after_manual_setting_change(&mut self) -> bool {
        self.auto_wallpaper_running = false;
        let now = Local::now();
        let current_minutes = now.hour() as u16 * 60 + now.minute() as u16;
        let scheduled_minutes = self.settings.auto_wallpaper_hour as u16 * 60
            + self.settings.auto_wallpaper_minute as u16;

        if current_minutes > scheduled_minutes {
            // 用户在设置面板里修改自动壁纸选项时，如果今天的计划时间已经过去，
            // 不应立刻补执行并退出程序；后台/开机启动时的补执行仍由普通轮询负责。
            self.settings.last_auto_wallpaper_date = Some(now.date_naive());
            false
        } else {
            self.settings.last_auto_wallpaper_date = None;
            current_minutes == scheduled_minutes
        }
    }

    fn set_auto_wallpaper_source(&mut self, source: AutoWallpaperSource, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_source = source;
        let should_check_now = self.prepare_auto_wallpaper_after_manual_setting_change();
        let _ = self.settings.save();
        self.set_status(
            format!(
                "Automatic wallpaper source: {}",
                source.label(self.settings.language)
            ),
            cx,
        );
        if self.settings.auto_wallpaper_enabled && should_check_now {
            self.check_scheduled_wallpaper(cx);
        }
    }

    fn set_auto_wallpaper_time(&mut self, hour: u8, minute: u8, cx: &mut Context<Self>) {
        self.settings.auto_wallpaper_hour = hour.min(23);
        self.settings.auto_wallpaper_minute = minute.min(59);
        let should_check_now = self.prepare_auto_wallpaper_after_manual_setting_change();
        let _ = self.settings.save();
        self.set_status(
            format!(
                "每日自动壁纸时间已设为 {:02}:{:02}",
                hour.min(23),
                minute.min(59)
            ),
            cx,
        );
        if self.settings.auto_wallpaper_enabled && should_check_now {
            self.check_scheduled_wallpaper(cx);
        }
    }

    pub fn trigger_auto_wallpaper_now(&mut self, cx: &mut Context<Self>) {
        self.run_auto_wallpaper(false, cx);
    }

    pub fn should_close_window(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.settings.background_resident_enabled {
            self.clear_thumbnail_cache(window, cx);
            hide_window_to_tray(window);
            self.set_status("已最小化到系统托盘，右键托盘图标可退出", cx);
            false
        } else {
            self.open_close_choice_dialog(window, cx);
            false
        }
    }

    fn open_close_choice_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_for_minimize = view.clone();
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
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    view_for_minimize.update(cx, |this, cx| {
                                        this.clear_thumbnail_cache(window, cx);
                                    });
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
        if self.auto_wallpaper_running || self.settings.last_auto_wallpaper_date == Some(today) {
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

        let scheduled_date = mark_today.then(|| Local::now().date_naive());
        if scheduled_date.is_some() {
            self.auto_wallpaper_running = true;
        }
        self.set_status(
            format!(
                "自动壁纸：正在使用{} - {}",
                self.settings
                    .auto_wallpaper_source
                    .label(self.settings.language),
                entry.date
            ),
            cx,
        );
        self.set_as_wallpaper_with_auto_mark(entry, scheduled_date, cx);
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

    fn set_download_resolution(&mut self, resolution: DownloadResolution, cx: &mut Context<Self>) {
        if self.settings.download_resolution == resolution {
            return;
        }
        self.settings.download_resolution = resolution;
        if let Err(err) = self.settings.save() {
            self.set_status(format!("保存分辨率设置失败: {err}"), cx);
            return;
        }
        self.set_status(
            format!(
                "{}: {}",
                self.settings.language.t("Global resolution"),
                resolution.status_label(self.settings.language)
            ),
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

    fn start_download(&mut self, entry: WallpaperEntry, cx: &mut Context<Self>) {
        let aria2 = self.aria2.clone();
        let aria2_start_lock = self.aria2_start_lock.clone();
        let http = self.http.clone();
        let date = entry.date;
        let resolution = self.settings.download_resolution;
        self.status = format!("正在下载 {} ...", entry.date).into();
        self.progress.insert(date, 0.0);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = run_download(
                &aria2,
                &aria2_start_lock,
                &http,
                &entry,
                resolution,
                &this,
                cx,
            )
            .await;
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
        let aria2_start_lock = self.aria2_start_lock.clone();
        let http = self.http.clone();
        let resolution = self.settings.download_resolution;
        self.batch_progress = Some(BatchProgress {
            total,
            ..Default::default()
        });
        self.set_status(format!("开始批量下载{label}: 共 {total} 张"), cx);

        cx.spawn(async move |this, cx| {
            let mut completed = 0usize;
            let mut skipped = 0usize;
            let mut failed = 0usize;
            let mut pending: VecDeque<WallpaperEntry> = entries.into();
            let mut active: Vec<(String, NaiveDate)> = Vec::new();

            let manager = match ensure_aria2(&aria2, &aria2_start_lock, &http).await {
                Ok(manager) => manager,
                Err(err) => {
                    let _ = this.update(cx, |this, cx| {
                        this.batch_progress = None;
                        this.set_status(format!("批量下载{label}失败: {err}"), cx);
                    });
                    return;
                }
            };

            const MAX_ACTIVE_DOWNLOADS: usize = 16;
            while !pending.is_empty() || !active.is_empty() {
                while active.len() < MAX_ACTIVE_DOWNLOADS {
                    let Some(entry) = pending.pop_front() else {
                        break;
                    };
                    let file_name = entry.file_name();
                    let existing = crate::paths::wallpapers_dir().map(|dir| dir.join(&file_name));
                    if existing.as_ref().is_ok_and(|path| path.exists()) {
                        completed += 1;
                        skipped += 1;
                        continue;
                    }

                    let date = entry.date;
                    let url = entry.download_url(resolution);
                    match manager.add_uri(&url, &file_name).await {
                        Ok(gid) => {
                            active.push((gid, date));
                            let _ = this.update(cx, |this, cx| {
                                this.progress.insert(date, 0.0);
                                cx.notify();
                            });
                        }
                        Err(_) => {
                            completed += 1;
                            failed += 1;
                        }
                    }
                }

                let mut progress_updates = Vec::new();
                let mut finished_dates = Vec::new();
                let mut index = 0;
                while index < active.len() {
                    let (gid, date) = &active[index];
                    let mut finished = false;
                    match manager.tell_status(gid).await {
                        Ok(status) => {
                            let state = status
                                .get("status")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("");
                            let completed_len: u64 = status
                                .get("completedLength")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0);
                            let total_len: u64 = status
                                .get("totalLength")
                                .and_then(serde_json::Value::as_str)
                                .and_then(|value| value.parse().ok())
                                .unwrap_or(0);
                            let percent = if total_len > 0 {
                                completed_len as f32 / total_len as f32 * 100.0
                            } else {
                                0.0
                            };
                            progress_updates.push((*date, percent));

                            match state {
                                "complete" => {
                                    completed += 1;
                                    finished = true;
                                }
                                "error" | "removed" => {
                                    completed += 1;
                                    failed += 1;
                                    finished = true;
                                }
                                _ => {}
                            }
                        }
                        Err(_) => {
                            completed += 1;
                            failed += 1;
                            finished = true;
                        }
                    }

                    if finished {
                        let (_, date) = active.swap_remove(index);
                        finished_dates.push(date);
                    } else {
                        index += 1;
                    }
                }

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
                            "批量下载{label}: {completed}/{total}（活动 {}，待提交 {}，跳过 {skipped}，失败 {failed}）",
                            active.len(),
                            pending.len()
                        ),
                        cx,
                    );
                });

                if !active.is_empty() {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                }
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
        self.set_as_wallpaper_with_auto_mark(entry, None, cx);
    }

    fn set_as_wallpaper_with_auto_mark(
        &mut self,
        entry: WallpaperEntry,
        auto_date: Option<NaiveDate>,
        cx: &mut Context<Self>,
    ) {
        let aria2 = self.aria2.clone();
        let aria2_start_lock = self.aria2_start_lock.clone();
        let http = self.http.clone();
        let date = entry.date;
        let target = self.settings.wallpaper_target.clone();
        let target_label = self.wallpaper_target_label();
        let resolution = self.settings.download_resolution;
        self.status = format!("正在将 {} 设置为{}壁纸...", entry.date, target_label).into();
        self.progress.insert(date, 0.0);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = run_download(
                &aria2,
                &aria2_start_lock,
                &http,
                &entry,
                resolution,
                &this,
                cx,
            )
            .await;
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
                let should_exit_after_auto = auto_date.is_some()
                    && outcome.is_ok()
                    && this.settings.auto_wallpaper_exit_after_done;
                if let Some(auto_date) = auto_date {
                    this.auto_wallpaper_running = false;
                    if outcome.is_ok() {
                        this.settings.last_auto_wallpaper_date = Some(auto_date);
                        let _ = this.settings.save();
                    }
                }
                match outcome {
                    Ok(()) => {
                        if should_exit_after_auto {
                            this.set_status("自动壁纸已设置完成，正在退出程序", cx);
                            cx.quit();
                        } else {
                            this.set_status(
                                format!("已将 {} 设置为{}壁纸", date, target_label),
                                cx,
                            );
                        }
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
        crate::local_thumbnails::remove(&path);
        self.cancel_downloaded_thumbnail_work();
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
        self.cancel_downloaded_thumbnail_work();
        for path in paths {
            crate::local_thumbnails::remove(&path);
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
        let candidate = if trimmed.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(trimmed))
        };
        let validated_dir = match &candidate {
            Some(dir) => std::fs::create_dir_all(dir).map(|()| dir.clone()),
            None => crate::paths::default_wallpapers_dir()
                .map_err(|err| std::io::Error::other(format!("创建默认下载目录失败: {err}"))),
        };
        let dir = match validated_dir {
            Ok(dir) => dir,
            Err(err) => {
                self.set_status(format!("下载路径无效: {err}"), cx);
                return;
            }
        };

        let previous = self.settings.download_dir.clone();
        self.settings.download_dir = candidate;
        if let Err(err) = self.settings.save() {
            self.settings.download_dir = previous;
            self.set_status(format!("保存设置失败: {err}"), cx);
            return;
        }
        self.cancel_downloaded_thumbnail_work();
        self.set_status(format!("已保存下载路径: {}", dir.display()), cx);

        let manager = self.aria2.borrow().clone();
        if let Some(manager) = manager {
            cx.spawn(async move |_this, _cx| {
                let _ = manager.change_download_dir(&dir).await;
            })
            .detach();
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
        self.set_status(
            format!("Theme: {}", preference.label(self.settings.language)),
            cx,
        );
        cx.notify();
    }

    fn set_language_preference(&mut self, language: LanguagePreference, cx: &mut Context<Self>) {
        let previous = self.settings.language;
        self.settings.language = language;
        if let Err(err) = self.settings.save() {
            self.settings.language = previous;
            self.set_status(format!("Failed to save language setting: {err}"), cx);
            return;
        }

        gpui_component::set_locale(language.gpui_locale());
        self.language_panel_open = false;
        self.settings_panel_open = false;
        self.set_status(
            format!("{}: {}", language.t("Language"), language.native_name()),
            cx,
        );
        cx.notify();
    }

    fn clear_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wallpaper_cache_result = crate::paths::cache_file().and_then(|path| {
            std::fs::remove_file(path).or_else(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(err.into())
                }
            })
        });
        let thumbnail_cache_result = crate::local_thumbnails::clear();
        self.downloaded_thumbnail_failures.clear();
        self.clear_downloaded_thumbnail_cache(window, cx);

        match (wallpaper_cache_result, thumbnail_cache_result) {
            (Ok(()), Ok(())) => {
                self.set_status("已清空壁纸列表与本地缩略图缓存，下次使用时会重新生成", cx)
            }
            (Err(err), _) => self.set_status(format!("清空壁纸列表缓存失败: {err}"), cx),
            (_, Err(err)) => self.set_status(format!("清空本地缩略图缓存失败: {err}"), cx),
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
        let date_str = entry.date_heading();
        let title = entry.title.clone();
        let url = entry.preview_url();
        let downloading = self.progress.contains_key(&entry.date);
        let (dialog_width, image_width, image_height) = preview_dialog_dimensions(window);
        let preview_cache = RetainAllImageCache::new(cx);
        let language = self.settings.language;

        window.open_dialog(cx, move |dialog, _window, cx| {
            let view_for_dl = view.clone();
            let view_for_wall = view.clone();
            let entry_for_dl = entry.clone();
            let entry_for_wall = entry.clone();
            let preview_cache = preview_cache.clone();

            dialog
                .title(date_str.clone())
                .w(px(dialog_width))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(image_cache(preview_cache).child(image_frame(
                            url.clone(),
                            image_width,
                            image_height,
                            language,
                            cx,
                        )))
                        .child(div().text_sm().line_clamp(2).child(title.clone())),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("preview-download")
                                .label(language.t("Download"))
                                .tooltip("下载当前高清壁纸到本地目录")
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
                                .label(language.t("Set as wallpaper"))
                                .tooltip("自动下载并按当前显示器设置应用为桌面壁纸")
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
        let language = self.settings.language;

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view_for_update = view.clone();
            let release_for_link = release.clone();
            let release_for_update = release.clone();
            let version = release.version.clone();

            dialog
                .title(language.t("Update available"))
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
                                .label(language.t("View release notes"))
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
                                .label(language.t("Later"))
                                .outline()
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            Button::new("update-now")
                                .label(language.t("Update now"))
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
        let aria2_start_lock = self.aria2_start_lock.clone();
        let http = self.http.clone();
        let progress = self.update_progress.clone();

        cx.spawn(async move |this, cx| {
            let result = run_update_download(
                &aria2,
                &aria2_start_lock,
                &http,
                &release,
                &progress,
                &this,
                cx,
            )
            .await;
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
fn open_about_dialog(language: LanguagePreference, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, cx| {
        dialog.title(language.t("About")).w(px(460.)).child(
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
                                .child(language.t("About subtitle")),
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
                                        .child(language.t("Version")),
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
                                        .child(language.t("Copyright")),
                                )
                                .child(div().font_bold().child(COPYRIGHT)),
                        ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_bold()
                                .child(language.t("Open source and credits")),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.t("About data sources")),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("about-bing-wallpaper")
                                        .label(language.t("Historical archive"))
                                        .outline()
                                        .on_click(|_, _, cx| {
                                            cx.open_url(
                                                "https://github.com/zxyongyo/bing-daily-wallpaper",
                                            );
                                        }),
                                )
                                .child(
                                    Button::new("about-repo")
                                        .label(language.t("Project home"))
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
    start_lock: &Rc<smol::lock::Mutex<()>>,
    http: &Arc<dyn HttpClient>,
) -> anyhow::Result<Rc<Aria2Manager>> {
    if let Some(existing) = aria2.borrow().clone() {
        return Ok(existing);
    }
    let _start_guard = start_lock.lock().await;
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
    aria2_start_lock: &Rc<smol::lock::Mutex<()>>,
    http: &Arc<dyn HttpClient>,
    entry: &WallpaperEntry,
    resolution: DownloadResolution,
    this: &WeakEntity<WallpaperLibrary>,
    cx: &mut AsyncApp,
) -> anyhow::Result<std::path::PathBuf> {
    let manager = ensure_aria2(aria2, aria2_start_lock, http).await?;
    let filename = entry.file_name();
    let date = entry.date;
    let url = entry.download_url(resolution);
    let gid = manager.add_uri(&url, &filename).await?;

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
    aria2_start_lock: &Rc<smol::lock::Mutex<()>>,
    http: &Arc<dyn HttpClient>,
    release: &crate::updater::ReleaseInfo,
    progress: &Rc<RefCell<Option<UpdateProgress>>>,
    this: &WeakEntity<WallpaperLibrary>,
    cx: &mut AsyncApp,
) -> anyhow::Result<std::path::PathBuf> {
    let manager = ensure_aria2(aria2, aria2_start_lock, http).await?;
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
                this.set_status("国内发行版下载失败，正在尝试备用下载地址...", cx);
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
            restore_window_to_default_placement(hwnd);
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        }
    } else {
        window.activate_window();
    }
}

unsafe fn restore_window_to_default_placement(hwnd: HWND) {
    let placement = window_sizing::default_window_placement();
    let _ = SetWindowPos(
        hwnd,
        None,
        placement.x,
        placement.y,
        placement.width,
        placement.height,
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

fn downloaded_image_info_text(path: &std::path::Path) -> String {
    let size = std::fs::metadata(path)
        .map(|metadata| format_bytes(metadata.len()))
        .unwrap_or_else(|_| "--".to_string());
    match image_dimensions(path) {
        Some((width, height)) => format!("{width}×{height} · {size}"),
        None => size,
    }
}

fn image_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    const HEADER_LIMIT: usize = 128 * 1024;
    let mut file = std::fs::File::open(path).ok()?;
    let mut data = Vec::with_capacity(HEADER_LIMIT);
    file.by_ref()
        .take(HEADER_LIMIT as u64)
        .read_to_end(&mut data)
        .ok()?;

    png_dimensions(&data)
        .or_else(|| jpeg_dimensions(&data))
        .or_else(|| webp_dimensions(&data))
}

fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let signature = b"\x89PNG\r\n\x1a\n";
    if data.len() < 24 || &data[..8] != signature || &data[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((width, height))
}

fn jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 4 || data[0] != 0xff || data[1] != 0xd8 {
        return None;
    }

    let mut index = 2usize;
    while index + 9 < data.len() {
        while index < data.len() && data[index] != 0xff {
            index += 1;
        }
        while index < data.len() && data[index] == 0xff {
            index += 1;
        }
        if index >= data.len() {
            break;
        }

        let marker = data[index];
        index += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01) {
            continue;
        }
        if index + 2 > data.len() {
            break;
        }

        let segment_len = u16::from_be_bytes([data[index], data[index + 1]]) as usize;
        if segment_len < 2 || index + segment_len > data.len() {
            break;
        }

        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && segment_len >= 7
        {
            let height = u16::from_be_bytes([data[index + 3], data[index + 4]]) as u32;
            let width = u16::from_be_bytes([data[index + 5], data[index + 6]]) as u32;
            return Some((width, height));
        }

        index += segment_len;
    }
    None
}

fn webp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 16 || &data[..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }

    let mut index = 12usize;
    while index + 8 <= data.len() {
        let chunk = &data[index..index + 4];
        let len = u32::from_le_bytes(data[index + 4..index + 8].try_into().ok()?) as usize;
        let payload = index + 8;
        if payload + len > data.len() {
            return None;
        }

        match chunk {
            b"VP8X" if len >= 10 => {
                let width = read_u24_le(&data[payload + 4..payload + 7])? + 1;
                let height = read_u24_le(&data[payload + 7..payload + 10])? + 1;
                return Some((width, height));
            }
            b"VP8 " if len >= 10 => {
                let width =
                    u16::from_le_bytes([data[payload + 6], data[payload + 7]]) as u32 & 0x3fff;
                let height =
                    u16::from_le_bytes([data[payload + 8], data[payload + 9]]) as u32 & 0x3fff;
                return Some((width, height));
            }
            b"VP8L" if len >= 5 && data[payload] == 0x2f => {
                let bits = u32::from_le_bytes([
                    data[payload + 1],
                    data[payload + 2],
                    data[payload + 3],
                    data[payload + 4],
                ]);
                let width = (bits & 0x3fff) + 1;
                let height = ((bits >> 14) & 0x3fff) + 1;
                return Some((width, height));
            }
            _ => {}
        }

        index = payload + len + (len % 2);
    }
    None
}

fn read_u24_le(data: &[u8]) -> Option<u32> {
    Some(
        data.first().copied()? as u32
            | (data.get(1).copied()? as u32) << 8
            | (data.get(2).copied()? as u32) << 16,
    )
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
        let language_panel_open = self.language_panel_open;
        let language = self.settings.language;

        let mut sidebar_menu = SidebarMenu::new();
        for (year, months) in years {
            let mut year_item = SidebarMenuItem::new(SharedString::from(language.year_label(year)))
                .icon(IconName::Calendar)
                .default_open(months.iter().any(|m| Some(&m.key) == selected_key.as_ref()))
                .click_to_toggle(true);

            let mut month_children = Vec::new();
            for month in months {
                let key = month.key.clone();
                let label = language.month_count_label(month.month, month.entries.len());
                let is_active = view_mode == ViewMode::MonthDetail
                    && selected_key.as_deref() == Some(month.key.as_str());
                month_children.push(
                    SidebarMenuItem::new(SharedString::from(label))
                        .active(is_active)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select(key.clone(), window, cx);
                        })),
                );
            }
            year_item = year_item.children(month_children);
            sidebar_menu = sidebar_menu.child(year_item);
        }

        let home_item = SidebarMenuItem::new(language.t("Home"))
            .icon(IconName::GalleryVerticalEnd)
            .active(view_mode == ViewMode::Home)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_view_mode(ViewMode::Home, window, cx);
            }));

        let resolution_children = DownloadResolution::ALL
            .into_iter()
            .map(|resolution| {
                SidebarMenuItem::new(SharedString::from(format!(
                    "{} {}",
                    resolution.label(language),
                    resolution.detail()
                )))
                .active(self.settings.download_resolution == resolution)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_download_resolution(resolution, cx);
                }))
            })
            .collect::<Vec<_>>();

        let resolution_item = SidebarMenuItem::new(SharedString::from(format!(
            "{}: {}",
            language.t("Global resolution"),
            self.settings.download_resolution.status_label(language)
        )))
        .icon(IconName::Frame)
        .default_open(!sidebar_collapsed)
        .click_to_toggle(true)
        .children(resolution_children);

        let favorites_item = SidebarMenuItem::new(language.t("Favorites"))
            .icon(IconName::Heart)
            .active(view_mode == ViewMode::Favorites)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_view_mode(ViewMode::Favorites, window, cx);
            }));

        let batch_download_item = SidebarMenuItem::new(language.t("Batch download"))
            .active(view_mode == ViewMode::DownloadBatch)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_view_mode(ViewMode::DownloadBatch, window, cx);
            }));

        let downloaded_item = SidebarMenuItem::new(language.t("Downloaded wallpapers"))
            .active(view_mode == ViewMode::Downloaded)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_view_mode(ViewMode::Downloaded, window, cx);
            }));

        let download_center_item = SidebarMenuItem::new(language.t("Download center"))
            .icon(IconName::FolderClosed)
            .default_open(matches!(
                view_mode,
                ViewMode::DownloadBatch | ViewMode::Downloaded
            ))
            .click_to_toggle(true)
            .children(vec![batch_download_item, downloaded_item]);

        let nav_menu = SidebarMenu::new()
            .child(home_item)
            .child(resolution_item)
            .child(favorites_item)
            .child(download_center_item);

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
                                            .child(div().font_bold().child("Bing Daily Wallpaper"))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .child(language.t("Wallpaper library")),
                                            ),
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
                    .child(SidebarGroup::new(language.t("Navigation")).child(nav_menu))
                    .when(!sidebar_collapsed, |this| {
                        this.child(SidebarGroup::new(language.t("Archive")).child(sidebar_menu))
                    })
                    .footer(
                        h_flex()
                            .gap_1()
                            .p_1()
                            .w_full()
                            .child(
                                Button::new("open-settings")
                                    .icon(IconName::Settings)
                                    .ghost()
                                    .small()
                                    .when(!settings_panel_open, |this| {
                                        this.tooltip(language.t("Settings"))
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_panel_open = !this.settings_panel_open;
                                        this.language_panel_open = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("open-language")
                                    .label(language.flag())
                                    .ghost()
                                    .small()
                                    .tooltip(language.t("Language"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.language_panel_open = !this.language_panel_open;
                                        this.settings_panel_open = false;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(match view_mode {
                ViewMode::Home => self.render_home_view(status, window, cx).into_any_element(),
                ViewMode::Favorites => self.render_favorites_view(status, cx).into_any_element(),
                ViewMode::DownloadBatch => self
                    .render_batch_download_view(status, cx)
                    .into_any_element(),
                ViewMode::Downloaded => self
                    .render_downloaded_view(status, window, cx)
                    .into_any_element(),
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
                        .child(self.render_settings_panel(window, cx)),
                )
            })
            .when(language_panel_open, |this| {
                let view_for_overlay = cx.entity();
                let view_for_menu = cx.entity();
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(
                            Button::new("language-panel-outside-close")
                                .label("")
                                .w_full()
                                .h_full()
                                .ghost()
                                .opacity(0.)
                                .on_click(move |_, _, cx| {
                                    view_for_overlay.update(cx, |this, cx| {
                                        this.language_panel_open = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    v_flex()
                        .absolute()
                        .left_3()
                        .bottom_12()
                        .gap_1()
                        .p_2()
                        .w(px(210.))
                        .rounded(cx.theme().radius_lg)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow_md()
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .font_bold()
                                .child(language.t("Language")),
                        )
                        .children(LanguagePreference::ALL.into_iter().map(move |candidate| {
                            let view = view_for_menu.clone();
                            Button::new(SharedString::from(format!("language-{candidate:?}")))
                                .label(format!(
                                    "{}  {}",
                                    candidate.flag(),
                                    if candidate == LanguagePreference::System {
                                        language.t("Follow system")
                                    } else {
                                        candidate.native_name()
                                    }
                                ))
                                .w_full()
                                .when(candidate == language, |this| this.primary())
                                .when(candidate != language, |this| this.ghost())
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.set_language_preference(candidate, cx);
                                    });
                                })
                        })),
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

    fn render_status_alert(&self, id: &'static str, status: &SharedString) -> Option<AnyElement> {
        let text = status.to_string();
        let normalized = text.to_lowercase();
        let is_error = text.contains("失败")
            || text.contains("错误")
            || text.contains("异常")
            || text.contains("失敗")
            || text.contains("실패")
            || normalized.contains("failed")
            || normalized.contains("error")
            || normalized.contains("erreur")
            || normalized.contains("échec")
            || normalized.contains("ошиб");
        let is_warning = text.starts_with("请选择")
            || text.contains("不能")
            || text.contains("没有可")
            || text.contains("为空");

        if is_error {
            Some(Alert::error(id, text).small().into_any_element())
        } else if is_warning {
            Some(Alert::warning(id, text).small().into_any_element())
        } else {
            None
        }
    }

    fn render_image_frame(
        &self,
        source: impl Into<ImageSource>,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .relative()
            .w(px(width))
            .h(px(height))
            .rounded(cx.theme().radius)
            .overflow_hidden()
            .bg(cx.theme().muted)
            .child(
                v_flex()
                    .absolute()
                    .inset_0()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(IconName::Frame).size_6())
                    .child(
                        div()
                            .text_xs()
                            .child(self.settings.language.t("Image loading...")),
                    ),
            )
            .child(
                img(source)
                    .absolute()
                    .inset_0()
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Cover),
            )
    }

    fn render_cached_thumbnail_frame(
        &self,
        source: impl Into<ImageSource>,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        image_cache(self.thumbnail_cache.clone())
            .child(self.render_image_frame(source, width, height, cx))
    }

    fn render_downloaded_thumbnail_frame(
        &self,
        source: &std::path::Path,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fingerprint = crate::local_thumbnails::fingerprint(source).ok();
        let thumbnail = fingerprint
            .as_ref()
            .and_then(crate::local_thumbnails::cached_for);
        if let Some(thumbnail) = thumbnail {
            image_cache(self.downloaded_thumbnail_cache.clone())
                .child(self.render_image_frame(thumbnail, width, height, cx))
                .into_any_element()
        } else {
            let placeholder = if crate::local_thumbnails::is_downloading(source) {
                "壁纸下载中..."
            } else if fingerprint
                .as_ref()
                .is_some_and(|fingerprint| self.downloaded_thumbnail_failures.contains(fingerprint))
            {
                "缩略图生成失败，请点击刷新重试"
            } else if fingerprint.is_none() {
                "无法读取本地图片"
            } else {
                "正在生成缩略图..."
            };
            div()
                .relative()
                .w(px(width))
                .h(px(height))
                .rounded(cx.theme().radius)
                .overflow_hidden()
                .bg(cx.theme().muted)
                .child(
                    v_flex()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(IconName::Frame).size_6())
                        .child(div().text_xs().child(placeholder)),
                )
                .into_any_element()
        }
    }

    fn render_settings_panel(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let panel_width = (viewport.width.as_f32() - 24.0).clamp(320.0, 760.0);
        let panel_height = (viewport.height.as_f32() - 72.0).clamp(280.0, 560.0);
        let section_width = if panel_width < 560.0 {
            136.0
        } else if panel_width < 640.0 {
            168.0
        } else {
            200.0
        };
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
        let auto_exit_after_done = self.settings.auto_wallpaper_exit_after_done;
        let auto_source = self.settings.auto_wallpaper_source;
        let auto_hour = self.settings.auto_wallpaper_hour;
        let auto_minute = self.settings.auto_wallpaper_minute;
        let language = self.settings.language;
        let about_language = language;

        let view_for_save = view.clone();
        let view_for_close = view.clone();
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
                .child(
                    div()
                        .text_sm()
                        .font_bold()
                        .child(language.t("Wallpaper download folder")),
                )
                .child(Input::new(&input_for_field))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.t("Download folder hint")),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("settings-open-dir")
                                .label(language.t("Open folder"))
                                .outline()
                                .small()
                                .on_click(move |_, _, cx| {
                                    let path = input_for_open.read(cx).value().to_string();
                                    open_in_explorer(&path);
                                }),
                        )
                        .child(
                            Button::new("settings-save-dir")
                                .label(language.t("Choose and save"))
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
                .child(div().text_sm().font_bold().child(language.t("Appearance")))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("theme-system")
                                .label(language.t("System"))
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
                                .label(language.t("Light"))
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
                                .label(language.t("Dark"))
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
                        .child(language.t("Theme hint")),
                )
                .into_any_element(),
            SettingsSection::WallpaperTarget => v_flex()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_bold()
                        .child(language.t("Multi-monitor wallpaper")),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("wallpaper-target-all")
                                .label(language.t("Sync all displays"))
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
                                .label(language.t("Refresh"))
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
                            .child(language.t("No display detected")),
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
                        .child(language.t("Display target hint")),
                )
                .into_any_element(),
            SettingsSection::Automation => {
                let view_for_startup = view.clone();
                let view_for_resident = view.clone();
                let view_for_auto = view.clone();
                let view_for_auto_exit = view.clone();
                let view_for_now = view.clone();
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .child(language.t("Automatic wallpaper")),
                    )
                    .child(
                        Checkbox::new("settings-startup-enabled")
                            .checked(startup_enabled)
                            .label(language.t("Startup"))
                            .on_click(move |_, _, cx| {
                                view_for_startup.update(cx, |this, cx| {
                                    this.toggle_startup_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        Checkbox::new("settings-resident-enabled")
                            .checked(resident_enabled)
                            .label(language.t("Run in background / show tray icon"))
                            .on_click(move |_, _, cx| {
                                view_for_resident.update(cx, |this, cx| {
                                    this.toggle_background_resident_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        Checkbox::new("settings-auto-wallpaper-enabled")
                            .checked(auto_enabled)
                            .label(language.t("Change wallpaper daily"))
                            .on_click(move |_, _, cx| {
                                view_for_auto.update(cx, |this, cx| {
                                    this.toggle_auto_wallpaper_enabled(cx);
                                });
                            }),
                    )
                    .child(
                        Checkbox::new("settings-auto-wallpaper-exit-after-done")
                            .checked(auto_exit_after_done)
                            .label(language.t("Exit after automatic wallpaper"))
                            .tooltip(language.t("Auto exit hint"))
                            .on_click(move |_, _, cx| {
                                view_for_auto_exit.update(cx, |this, cx| {
                                    this.toggle_auto_wallpaper_exit_after_done(cx);
                                });
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "Schedule: {:02}:{:02}, source: {}{}.",
                                auto_hour,
                                auto_minute,
                                match auto_source {
                                    AutoWallpaperSource::Latest =>
                                        language.t("Latest daily wallpaper"),
                                    AutoWallpaperSource::RandomAll =>
                                        language.t("Random from all history"),
                                    AutoWallpaperSource::RandomFavorites =>
                                        language.t("Random from favorites"),
                                },
                                if auto_exit_after_done {
                                    "; exit after success"
                                } else {
                                    ""
                                }
                            )),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .child(language.t("Wallpaper source")),
                    )
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            [
                                AutoWallpaperSource::Latest,
                                AutoWallpaperSource::RandomAll,
                                AutoWallpaperSource::RandomFavorites,
                            ]
                            .into_iter()
                            .map(|source| {
                                let view_for_source = view.clone();
                                Button::new(SharedString::from(format!("auto-source-{source:?}")))
                                    .label(match source {
                                        AutoWallpaperSource::Latest => {
                                            language.t("Latest daily wallpaper")
                                        }
                                        AutoWallpaperSource::RandomAll => {
                                            language.t("Random from all history")
                                        }
                                        AutoWallpaperSource::RandomFavorites => {
                                            language.t("Random from favorites")
                                        }
                                    })
                                    .small()
                                    .when(auto_source == source, |this| this.primary())
                                    .when(auto_source != source, |this| this.outline())
                                    .on_click(move |_, _, cx| {
                                        view_for_source.update(cx, |this, cx| {
                                            this.set_auto_wallpaper_source(source, cx);
                                        });
                                    })
                            }),
                        ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_bold()
                            .child(language.t("Execution time")),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .w(px(120.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(language.t("Hour")),
                                    )
                                    .child(
                                        div()
                                            .id("auto-hour-scroll")
                                            .h(px(180.))
                                            .overflow_y_scroll()
                                            .on_scroll_wheel(
                                                |_event: &ScrollWheelEvent, _window, cx| {
                                                    cx.stop_propagation()
                                                },
                                            )
                                            .rounded(cx.theme().radius)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .p_1()
                                            .children((0u8..24).map(|hour| {
                                                let selected = auto_hour == hour;
                                                let view_for_time = view.clone();
                                                Button::new(SharedString::from(format!(
                                                    "auto-hour-{hour}"
                                                )))
                                                .label(format!("{:02}", hour))
                                                .small()
                                                .w_full()
                                                .when(selected, |this| this.primary())
                                                .when(!selected, |this| this.ghost())
                                                .on_click(move |_, _, cx| {
                                                    view_for_time.update(cx, |this, cx| {
                                                        this.set_auto_wallpaper_time(
                                                            hour,
                                                            auto_minute,
                                                            cx,
                                                        );
                                                    });
                                                })
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .w(px(120.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(language.t("Minute")),
                                    )
                                    .child(
                                        div()
                                            .id("auto-minute-scroll")
                                            .h(px(180.))
                                            .overflow_y_scroll()
                                            .on_scroll_wheel(
                                                |_event: &ScrollWheelEvent, _window, cx| {
                                                    cx.stop_propagation()
                                                },
                                            )
                                            .rounded(cx.theme().radius)
                                            .border_1()
                                            .border_color(cx.theme().border)
                                            .p_1()
                                            .children((0u8..60).map(|minute| {
                                                let selected = auto_minute == minute;
                                                let view_for_time = view.clone();
                                                Button::new(SharedString::from(format!(
                                                    "auto-minute-{minute}"
                                                )))
                                                .label(format!("{:02}", minute))
                                                .small()
                                                .w_full()
                                                .when(selected, |this| this.primary())
                                                .when(!selected, |this| this.ghost())
                                                .on_click(move |_, _, cx| {
                                                    view_for_time.update(cx, |this, cx| {
                                                        this.set_auto_wallpaper_time(
                                                            auto_hour, minute, cx,
                                                        );
                                                    });
                                                })
                                            })),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(language.t("Current selection")),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_bold()
                                            .child(format!("{:02}:{:02}", auto_hour, auto_minute)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(language.t("Time selection hint")),
                                    ),
                            ),
                    )
                    .child(
                        Button::new("auto-wallpaper-now")
                            .label(language.t("Change once now"))
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
                            .child(language.t("Automatic wallpaper hint")),
                    )
                    .into_any_element()
            }
            SettingsSection::Maintenance => v_flex()
                .gap_2()
                .child(div().text_sm().font_bold().child(language.t("Maintenance")))
                .child(
                    Button::new("settings-clear-cache")
                        .label(language.t("Clear wallpaper cache"))
                        .outline()
                        .w_full()
                        .on_click(move |_, window, cx| {
                            view_for_clear.update(cx, |this, cx| {
                                this.clear_cache(window, cx);
                            });
                        }),
                )
                .child(
                    Button::new("settings-check-update")
                        .label(language.t("Check for updates"))
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
                        .label(language.t("About"))
                        .outline()
                        .w_full()
                        .on_click(move |_, window, cx| {
                            open_about_dialog(about_language, window, cx);
                        }),
                )
                .into_any_element(),
        };

        h_flex()
            .relative()
            .w(px(panel_width))
            .h(px(panel_height))
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
                    .w(px(section_width))
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
                    .child(div().font_bold().child(language.t("Settings")))
                    .child(self.render_settings_section_header(
                        "settings-section-download",
                        language.t("Download path"),
                        SettingsSection::DownloadDir,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-appearance",
                        language.t("Appearance"),
                        SettingsSection::Appearance,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-wallpaper-target",
                        language.t("Multi-monitor wallpaper"),
                        SettingsSection::WallpaperTarget,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-automation",
                        language.t("Automatic wallpaper"),
                        SettingsSection::Automation,
                        cx,
                    ))
                    .child(self.render_settings_section_header(
                        "settings-section-maintenance",
                        language.t("Maintenance"),
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
            .child(
                div().absolute().top_2().right_2().child(
                    Button::new("settings-panel-close")
                        .icon(
                            Icon::empty()
                                .path("icons/close.svg")
                                .size_4()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .ghost()
                        .small()
                        .tooltip(language.t("Close settings"))
                        .on_click(move |_, _, cx| {
                            view_for_close.update(cx, |this, cx| {
                                this.settings_panel_open = false;
                                cx.notify();
                            });
                        }),
                ),
            )
    }

    fn home_grid_columns(&self, window: &Window) -> usize {
        let sidebar_width = if self.sidebar_collapsed {
            px(SIDEBAR_COLLAPSED_WIDTH)
        } else {
            px(SIDEBAR_EXPANDED_WIDTH)
        };
        let available_width = window.viewport_size().width - sidebar_width - px(56.0);
        let columns = ((available_width + px(HOME_GRID_GAP))
            / px(HOME_GRID_CARD_WIDTH + HOME_GRID_GAP))
        .floor() as usize;
        columns.max(1)
    }

    fn render_home_view(
        &self,
        status: SharedString,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let total = self.flat_entries.len();
        let language = self.settings.language;
        let columns = self.home_grid_columns(window);
        let rows: Rc<Vec<Vec<WallpaperEntry>>> = Rc::new(
            self.flat_entries
                .chunks(columns)
                .map(|chunk| chunk.to_vec())
                .collect(),
        );
        let item_sizes = Rc::new(
            (0..rows.len())
                .map(|_| size(px(1.), px(HOME_GRID_ROW_HEIGHT)))
                .collect::<Vec<_>>(),
        );

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(div().font_bold().text_lg().flex_shrink_0().child(format!(
                        "{} · {} ({total})",
                        language.t("Home"),
                        language.t("Recent wallpapers")
                    )))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(status.clone()),
                    )
                    .child(
                        Button::new("home-refresh-wallpaper-list")
                            .label(language.t("Refresh wallpaper list"))
                            .small()
                            .outline()
                            .flex_shrink_0()
                            .tooltip("重新从远程数据源获取壁纸列表；网络不可用时会继续使用内置列表")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_wallpaper_list(cx);
                            })),
                    ),
            )
            .when_some(
                self.render_status_alert("home-status-alert", &status),
                |this, alert| this.child(alert),
            )
            .child(
                div()
                    .id("home-scroll-wrap")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("home-virtual-scroll")
                            .relative()
                            .size_full()
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    "home-wallpaper-rows",
                                    item_sizes,
                                    move |view, visible_range, _window, cx| {
                                        visible_range
                                            .filter_map(|row_index| rows.get(row_index).cloned())
                                            .map(|row| {
                                                h_flex().gap_4().pb_4().children(
                                                    row.into_iter().map(|entry| {
                                                        view.render_home_card(entry, cx)
                                                    }),
                                                )
                                            })
                                            .collect()
                                    },
                                )
                                .track_scroll(&self.home_scroll_handle)
                                .pr_2(),
                            ),
                    )
                    .vertical_scrollbar(&self.home_scroll_handle)
                    .child(
                        div().absolute().bottom_6().right_6().child(
                            Button::new("home-back-to-top")
                                .icon(IconName::ArrowUp)
                                .ghost()
                                .opacity(0.6)
                                .tooltip(language.t("Back to top"))
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
        let language = self.settings.language;

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .flex_shrink_0()
                            .child(format!("{} ({count})", language.t("Favorites"))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(status.clone()),
                    ),
            )
            .when_some(
                self.render_status_alert("favorites-status-alert", &status),
                |this, alert| this.child(alert),
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
                            .child(
                                div()
                                    .text_lg()
                                    .font_bold()
                                    .child(language.t("No favorite wallpapers yet")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.t("Favorites empty hint")),
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
        let language = self.settings.language;

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
                    .gap_3()
                    .child(div().font_bold().text_lg().flex_shrink_0().child(format!(
                        "{} · {}",
                        language.t("Download center"),
                        language.t("Batch download")
                    )))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(status.clone()),
                    ),
            )
            .when_some(
                self.render_status_alert("batch-status-alert", &status),
                |this, alert| this.child(alert),
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
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .child(language.t("Quick download")),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("batch-center-all")
                                            .label(language.t("All history"))
                                            .tooltip("下载当前列表中的全部历史壁纸")
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
                                            .label(language.t("Current month"))
                                            .tooltip("下载左侧当前选中月份的壁纸")
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
                                            .label(language.t("Favorites"))
                                            .tooltip("下载我的收藏中的全部壁纸")
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
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .child(language.t("Download by date range")),
                            )
                            .child(
                                DatePicker::new(&batch_range_picker)
                                    .placeholder(language.t("Select a date range"))
                                    .cleanable(true)
                                    .disabled(date_limits.is_none())
                                    .w(px(360.)),
                            )
                            .child(
                                Button::new("batch-center-date-range")
                                    .label(language.t("Download date range"))
                                    .tooltip("下载日历中选中的日期范围壁纸")
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

    fn queue_downloaded_thumbnails(
        &mut self,
        paths: impl IntoIterator<Item = std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        for path in paths {
            if crate::local_thumbnails::is_downloading(&path) {
                continue;
            }
            let Ok(fingerprint) = crate::local_thumbnails::fingerprint(&path) else {
                continue;
            };
            if crate::local_thumbnails::cached_for(&fingerprint).is_some()
                || self.downloaded_thumbnail_failures.contains(&fingerprint)
                || !self
                    .downloaded_thumbnail_pending
                    .insert(fingerprint.clone())
            {
                continue;
            }
            self.downloaded_thumbnail_queue.push_back((
                path,
                fingerprint,
                self.downloaded_thumbnail_epoch,
            ));
        }

        if self.downloaded_thumbnail_worker_running || self.downloaded_thumbnail_queue.is_empty() {
            return;
        }
        self.downloaded_thumbnail_worker_running = true;

        cx.spawn(async move |this, cx| loop {
            let (path, fingerprint, epoch) = match this.update(cx, |this, _cx| {
                let request = this.downloaded_thumbnail_queue.pop_front();
                if request.is_none() {
                    this.downloaded_thumbnail_worker_running = false;
                }
                request
            }) {
                Ok(Some(request)) => request,
                Ok(None) | Err(_) => return,
            };
            let source = path.clone();
            let expected = fingerprint.clone();
            let result = cx
                .background_executor()
                .spawn(async move { crate::local_thumbnails::ensure(&source, &expected) })
                .await;

            let should_continue = match this.update(cx, |this, cx| {
                this.downloaded_thumbnail_pending.remove(&fingerprint);
                if epoch == this.downloaded_thumbnail_epoch {
                    if let Err(err) = &result {
                        if !crate::local_thumbnails::is_downloading(&path)
                            && crate::local_thumbnails::fingerprint(&path).ok().as_ref()
                                == Some(&fingerprint)
                        {
                            this.downloaded_thumbnail_failures
                                .insert(fingerprint.clone());
                        }
                        log::warn!("生成本地壁纸缩略图失败: {err:#}");
                    }
                    cx.notify();
                }
                let should_continue = !this.downloaded_thumbnail_queue.is_empty();
                if !should_continue {
                    this.downloaded_thumbnail_worker_running = false;
                }
                should_continue
            }) {
                Ok(should_continue) => should_continue,
                Err(_) => return,
            };

            if !should_continue {
                return;
            }
        })
        .detach();
    }

    fn downloaded_grid_columns(&self, window: &Window) -> usize {
        let sidebar_width = if self.sidebar_collapsed {
            px(SIDEBAR_COLLAPSED_WIDTH)
        } else {
            px(SIDEBAR_EXPANDED_WIDTH)
        };
        let available_width = window.viewport_size().width - sidebar_width - px(56.0);
        let columns = ((available_width + px(DOWNLOADED_GRID_GAP))
            / px(DOWNLOADED_GRID_CARD_WIDTH + DOWNLOADED_GRID_GAP))
        .floor() as usize;
        columns.max(1)
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
                    .filter(|path| path.is_file() && !crate::local_thumbnails::is_downloading(path))
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
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let files = self.downloaded_files();
        let count = files.len();
        let language = self.settings.language;
        let all_selected = count > 0 && files.iter().all(|p| self.downloaded_selected.contains(p));
        let selected_count = self.downloaded_selected.len();
        let dir_display = crate::paths::wallpapers_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        let columns = self.downloaded_grid_columns(window);
        let rows: Rc<Vec<Vec<std::path::PathBuf>>> =
            Rc::new(files.chunks(columns).map(|chunk| chunk.to_vec()).collect());
        let item_sizes = Rc::new(
            (0..rows.len())
                .map(|_| size(px(1.), px(DOWNLOADED_GRID_ROW_HEIGHT)))
                .collect::<Vec<_>>(),
        );

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
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(div().font_bold().text_lg().flex_shrink_0().child(format!(
                                "{} · {} ({count})",
                                language.t("Download center"),
                                language.t("Downloaded wallpapers")
                            )))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(status.clone()),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("downloaded-select-all")
                                    .tooltip("全选或取消全选当前下载目录中的壁纸")
                                    .label(if all_selected {
                                        language.t("Clear selection")
                                    } else {
                                        language.t("Select all")
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
                                    .tooltip("删除当前勾选的本地壁纸文件")
                                    .label(format!(
                                        "{} ({selected_count})",
                                        language.t("Delete selected")
                                    ))
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
                                    .tooltip("重新扫描下载目录")
                                    .label(language.t("Refresh"))
                                    .outline()
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.downloaded_thumbnail_failures.clear();
                                        this.clear_downloaded_thumbnail_cache(window, cx);
                                        this.downloaded_scroll_handle
                                            .set_offset(point(px(0.), px(0.)));
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("downloaded-open-dir")
                                    .tooltip("在资源管理器中打开当前壁纸下载目录")
                                    .label(language.t("Open folder"))
                                    .outline()
                                    .small()
                                    .on_click(move |_, _, _cx| {
                                        open_in_explorer(&dir_display);
                                    }),
                            ),
                    ),
            )
            .when_some(
                self.render_status_alert("downloaded-status-alert", &status),
                |this, alert| this.child(alert),
            )
            .child(if files.is_empty() {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_bold()
                            .child(language.t("No downloaded wallpapers yet")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("在首页、归档或批量下载中下载壁纸后会显示在这里"),
                    )
                    .into_any_element()
            } else {
                div()
                    .id("downloaded-scroll-wrap")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "downloaded-wallpaper-rows",
                            item_sizes,
                            move |view, visible_range, _window, cx| {
                                visible_range
                                    .filter_map(|row_index| rows.get(row_index).cloned())
                                    .map(|row| {
                                        view.queue_downloaded_thumbnails(row.iter().cloned(), cx);
                                        h_flex().gap_4().pb_4().children(
                                            row.into_iter()
                                                .map(|path| view.render_downloaded_card(path, cx)),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.downloaded_scroll_handle)
                        .pr_2(),
                    )
                    .vertical_scrollbar(&self.downloaded_scroll_handle)
                    .into_any_element()
            })
    }

    /// 已下载壁纸画廊中的单张卡片：点击图片本身预览（弹窗内可设为桌面壁纸/删除），
    /// 左上角勾选框用于批量选择，鼠标悬停时图片底部浮现“设为桌面壁纸”/删除按钮。
    fn render_downloaded_card(
        &self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
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
        let info_text = downloaded_image_info_text(&path);

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
                    .child(self.render_downloaded_thumbnail_frame(&path, 220., 124., cx))
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
                                .tooltip("选择这张本地壁纸用于批量删除")
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
                                        .label(language.t("Set as wallpaper"))
                                        .tooltip("将这张本地图片设置为桌面壁纸")
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
                                        .tooltip(language.t("Delete"))
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
                v_flex()
                    .gap_1()
                    .child(div().text_xs().truncate().child(name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(info_text),
                    ),
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
        let (dialog_width, image_width, image_height) = preview_dialog_dimensions(window);
        let preview_cache = RetainAllImageCache::new(cx);
        let language = self.settings.language;

        window.open_dialog(cx, move |dialog, _window, cx| {
            let view_for_delete = view.clone();
            let view_for_wall = view.clone();
            let path_for_delete = path.clone();
            let path_for_wall = path.clone();
            let preview_cache = preview_cache.clone();

            dialog
                .title(
                    path_for_title
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
                .w(px(dialog_width))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(image_cache(preview_cache).child(image_frame(
                            path.clone(),
                            image_width,
                            image_height,
                            language,
                            cx,
                        )))
                        .child(div().text_sm().truncate().child(name.clone())),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("local-preview-delete")
                                .label(language.t("Delete"))
                                .tooltip("删除这张本地壁纸文件")
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
                                .label(language.t("Set as wallpaper"))
                                .tooltip("将这张本地图片设置为桌面壁纸")
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
        let language = self.settings.language;

        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_3()
            .p_4()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .flex_shrink_0()
                            .child(match &content {
                                Some(group) => language.year_month_label(group.year, group.month),
                                None => language.t("Select a month from the sidebar").to_string(),
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(status.clone()),
                    ),
            )
            .when_some(
                self.render_status_alert("month-status-alert", &status),
                |this, alert| this.child(alert),
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
        let language = self.settings.language;
        let group_name: SharedString = format!("home-card-{}", entry.date).into();
        let date_str = entry.date_heading();
        let entry_for_preview = entry.clone();
        let entry_for_wallpaper = entry.clone();
        let favorite_date = entry.date;
        let is_favorite = self.favorites.contains(&entry.date);
        let progress = self.progress.get(&entry.date).copied();

        v_flex()
            .group(group_name.clone())
            .w(px(260.))
            .h(px(224.))
            .gap_2()
            .child(
                div()
                    .relative()
                    .w(px(260.))
                    .h(px(146.))
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(self.render_cached_thumbnail_frame(
                        entry.thumbnail_url(),
                        260.,
                        146.,
                        cx,
                    ))
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
                                        .label(language.t("Set as wallpaper"))
                                        .tooltip("自动下载并按当前显示器设置应用为桌面壁纸")
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
                                            language.t("Cancel favorite")
                                        } else {
                                            language.t("Favorite")
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
                    .h(px(62.))
                    .gap_1()
                    .child(div().text_sm().font_bold().child(date_str))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .line_clamp(2)
                            .child(entry.title.clone()),
                    ),
            )
    }

    fn render_entry_card(&self, entry: WallpaperEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.language;
        let entry_for_download = entry.clone();
        let entry_for_preview = entry.clone();
        let entry_for_wallpaper = entry.clone();
        let favorite_date = entry.date;
        let is_favorite = self.favorites.contains(&entry.date);
        let date_str = entry.date_heading();
        let progress = self.progress.get(&entry.date).copied();

        h_flex()
            .gap_3()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .child(self.render_cached_thumbnail_frame(entry.thumbnail_url(), 220., 124., cx))
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
                            .flex_wrap()
                            .child(
                                Button::new(SharedString::from(format!("dl-{}", entry.date)))
                                    .label(language.t("Download"))
                                    .tooltip("下载当前高清壁纸到本地目录")
                                    .disabled(progress.is_some())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.start_download(entry_for_download.clone(), cx);
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("preview-{}", entry.date)))
                                    .label(language.t("Preview"))
                                    .tooltip("打开高清大图预览")
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
                                    .label(language.t("Set as wallpaper"))
                                    .tooltip("自动下载并按当前显示器设置应用为桌面壁纸")
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
                                        language.t("Cancel favorite")
                                    } else {
                                        language.t("Favorite")
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
