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
use crate::settings::AppSettings;
use crate::wallpaper_setter;
use chrono::NaiveDate;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants as _;
use gpui_component::dialog::DialogFooter;
use gpui_component::input::{Input, InputState};
use gpui_component::progress::Progress;
use gpui_component::sidebar::{
    Sidebar, SidebarCollapsible, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem,
    SidebarToggleButton,
};
use gpui_component::{
    button::Button,
    Root, WindowExt as _,
};
use gpui_component::*;
use http_client::HttpClient;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// 软件版权/作者署名，展示于侧边栏底部。
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
    /// 点击左侧某个年/月条目后展示的旧版列表视图。
    MonthDetail,
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
    /// 首页网格当前已展示的壁纸数量（初始 [`HOME_PAGE_SIZE`] 张，滚动到底部后递增）。
    home_loaded_count: usize,
    /// 首页网格滚动容器的滚动状态句柄，用于判断是否已接近底部。
    home_scroll_handle: ScrollHandle,
    /// 侧边导航栏是否处于折叠（仅图标）状态。
    sidebar_collapsed: bool,
    /// 持久化的应用设置（目前只有自定义下载路径）。
    settings: AppSettings,
    /// 设置面板中"下载路径"输入框的状态。
    settings_dir_input: Entity<InputState>,
}

impl WallpaperLibrary {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = AppSettings::load();
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

        Self {
            groups: Vec::new(),
            flat_entries: Vec::new(),
            view_mode: ViewMode::Home,
            selected_key: None,
            status: "正在加载壁纸列表...".into(),
            aria2: Rc::new(RefCell::new(None)),
            http: cx.http_client(),
            progress: HashMap::new(),
            home_loaded_count: HOME_PAGE_SIZE,
            home_scroll_handle: ScrollHandle::new(),
            sidebar_collapsed: false,
            settings,
            settings_dir_input,
        }
    }

    /// 导出内部持有的 aria2 管理器共享句柄，供应用退出时优雅关闭使用（见 `main.rs`）。
    pub fn aria2_handle(&self) -> Rc<RefCell<Option<Rc<Aria2Manager>>>> {
        self.aria2.clone()
    }

    /// 使用最新抓取到的壁纸条目刷新界面状态。
    pub fn set_entries(&mut self, entries: Vec<WallpaperEntry>, cx: &mut Context<Self>) {
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

    fn set_as_wallpaper(&mut self, entry: WallpaperEntry, cx: &mut Context<Self>) {
        let aria2 = self.aria2.clone();
        let http = self.http.clone();
        let date = entry.date;
        self.status = format!("正在设置 {} 的壁纸...", entry.date).into();
        self.progress.insert(date, 0.0);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = run_download(&aria2, &http, &entry, &this, cx).await;
            let outcome = match result {
                Ok(path) => wallpaper_setter::set_wallpaper(&path),
                Err(err) => Err(err),
            };
            let _ = this.update(cx, |this, cx| {
                this.progress.remove(&date);
                match outcome {
                    Ok(()) => this.set_status(format!("已将 {} 设置为桌面壁纸", date), cx),
                    Err(err) => this.set_status(format!("设置壁纸失败: {err}"), cx),
                }
            });
        })
        .detach();
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

    /// 打开"设置"对话框：目前只支持配置壁纸下载保存路径。
    fn open_settings_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.settings_dir_input.clone();
        let view = cx.entity();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let input_for_field = input.clone();
            let input_for_explorer = input.clone();
            let input_for_save = input.clone();
            let view_for_save = view.clone();

            dialog
                .title("设置")
                .w(px(480.))
                .child(
                    v_flex()
                        .gap_3()
                        .p_4()
                        .child(div().text_sm().font_bold().child("壁纸下载保存路径"))
                        .child(Input::new(&input_for_field))
                        .child(
                            div()
                                .text_xs()
                                .opacity(0.6)
                                .child("留空则使用默认目录；保存后若路径不存在会自动创建。"),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .justify_between()
                        .child(
                            Button::new("open-download-dir")
                                .label("在资源管理器中打开")
                                .outline()
                                .on_click(move |_, _, cx| {
                                    let path = input_for_explorer.read(cx).value().to_string();
                                    open_in_explorer(&path);
                                }),
                        )
                        .child(
                            Button::new("save-settings")
                                .label("保存")
                                .primary()
                                .on_click(move |_, window, cx| {
                                    let path_str = input_for_save.read(cx).value().to_string();
                                    view_for_save.update(cx, |this, cx| {
                                        this.apply_download_dir(path_str, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    /// 打开"预览图片"对话框：展示原始高清大图，底部提供下载/设为壁纸按钮。
    ///
    /// 注意：`downloading` 必须在调用 `window.open_dialog` **之前**，从 `&self` 同步快照
    /// 一次，而不能在对话框的 builder 闭包内部通过 `view.read(cx)` 读取——因为
    /// `render_dialog_layer` 是在 `WallpaperLibrary::render` 自身的渲染过程中被调用的，
    /// 此时本 Entity 正处于"正在被更新"状态，重入读取会触发 GPUI 的
    /// `cannot read ... while it is already being updated` panic（应用直接崩溃）。
    fn open_preview_dialog(&self, entry: WallpaperEntry, window: &mut Window, cx: &mut Context<Self>) {
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
                        .child(
                            img(url.clone())
                                .w(px(800.))
                                .h(px(450.))
                                .rounded(px(6.)),
                        )
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

impl Render for WallpaperLibrary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let years = self.years();
        let selected_key = self.selected_key.clone();
        let view_mode = self.view_mode;
        let sidebar_collapsed = self.sidebar_collapsed;

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
                let is_active =
                    view_mode == ViewMode::MonthDetail && selected_key.as_deref() == Some(month.key.as_str());
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

        let status = self.status.clone();

        let title_bar = TitleBar::new().child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .font_bold()
                .child("必应每日壁纸库"),
        );

        let main_row = h_flex()
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
                                            .child(div().font_bold().child("必应每日壁纸库"))
                                            .child(div().text_xs().child("按年月浏览历史壁纸")),
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
                    .child(SidebarGroup::new("导航").child(SidebarMenu::new().child(home_item)))
                    .when(!sidebar_collapsed, |this| {
                        this.child(SidebarGroup::new("归档").child(sidebar_menu))
                    })
                    .footer(
                        v_flex()
                            .gap_2()
                            .p_2()
                            .w_full()
                            .child(
                                Button::new("open-settings")
                                    .icon(IconName::Settings)
                                    .ghost()
                                    .small()
                                    .tooltip("设置")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_settings_dialog(window, cx);
                                    })),
                            )
                            .when(!sidebar_collapsed, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(COPYRIGHT),
                                )
                            }),
                    ),
            )
            .child(match view_mode {
                ViewMode::Home => self.render_home_view(status, cx).into_any_element(),
                ViewMode::MonthDetail => self
                    .render_month_view(selected_key, status, cx)
                    .into_any_element(),
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
                    .id("home-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.home_scroll_handle)
                    .on_scroll_wheel(cx.listener(|this, _event: &ScrollWheelEvent, _window, cx| {
                        this.maybe_load_more_home(cx);
                    }))
                    .child(
                        div().flex().flex_wrap().gap_4().children(
                            entries
                                .into_iter()
                                .map(|entry| self.render_home_card(entry, cx)),
                        ),
                    ),
            )
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
                    .child(
                        div()
                            .font_bold()
                            .text_lg()
                            .child(match &content {
                                Some(group) => {
                                    format!("{}年{:02}月", group.year, group.month)
                                }
                                None => "请选择左侧月份".to_string(),
                            }),
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

    /// 首页网格中的单张壁纸卡片：鼠标悬停时图片底部浮现"预览图片"按钮
    /// （纯 CSS 式的 group-hover 实现，不依赖任何应用状态/`cx.notify()`，
    /// 保证滚动与悬停都足够流畅）。
    fn render_home_card(&self, entry: WallpaperEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let group_name: SharedString = format!("home-card-{}", entry.date).into();
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        let entry_for_preview = entry.clone();

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
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .p_2()
                            .opacity(0.)
                            .group_hover(group_name.clone(), |style| style.opacity(1.))
                            .bg(hsla(0., 0., 0., 0.55))
                            .child(
                                Button::new(SharedString::from(format!(
                                    "home-preview-{}",
                                    entry.date
                                )))
                                .label("预览图片")
                                .small()
                                .w_full()
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_preview_dialog(entry_for_preview.clone(), window, cx);
                                })),
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

    fn render_entry_card(
        &self,
        entry: WallpaperEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let entry_for_download = entry.clone();
        let entry_for_preview = entry.clone();
        let entry_for_wallpaper = entry.clone();
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
