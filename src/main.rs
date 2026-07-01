//! 必应每日壁纸库 —— 程序入口。
//!
//! 启动流程：
//! 1. 检测/请求管理员权限（见 [`elevate`] 模块）；
//! 2. 检测是否已有另一个实例在运行（见 [`single_instance`] 模块）；
//! 3. 初始化日志；
//! 4. 启动 GPUI 应用，加载本地缓存的壁纸列表（如果存在）用于快速展示；
//! 5. 后台异步拉取 `bing-wallpaper.md` 获取最新的完整历史壁纸列表并写入缓存；
//! 6. 启动一个每 30 分钟轮询一次的后台任务，检测是否有新的一天的壁纸发布。
//!
//! 使用 `windows` 子系统构建（仅在 release 构建时生效，见下方 `windows_subsystem`
//! 属性），避免最终发布的 exe 启动时弹出黑色控制台窗口；debug 构建/`cargo test` 时
//! 仍保留控制台，便于开发调试查看日志输出。应用图标通过 `build.rs` + `ico/icon.rc`
//! 以数字资源 ID `1` 嵌入（`gpui` 在 Windows 上按此固定 ID 查找窗口/任务栏图标，
//! 详见 AGENTS.md）。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod downloader;
mod elevate;
mod fetcher;
mod model;
mod paths;
mod settings;
mod single_instance;
mod ui;
mod updater;
mod wallpaper_setter;

use gpui::*;
use gpui_component::{Root, Theme};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use ui::WallpaperLibrary;

/// 后台检查更新的时间间隔。
const REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);

fn main() {
    if !elevate::ensure_elevated() {
        // 已尝试以管理员身份重新启动自身，当前（非提权）进程直接退出。
        return;
    }

    if !single_instance::ensure_single_instance() {
        // 已有另一个实例在运行，已尝试将其窗口带到前台，当前进程直接退出。
        return;
    }

    env_logger::init();

    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx: &mut App| {
        let http_client = reqwest_client::ReqwestClient::user_agent(paths::APP_NAME)
            .expect("创建 HTTP 客户端失败");
        cx.set_http_client(Arc::new(http_client));

        gpui_component::init(cx);

        // `appears_transparent: true` 关闭 Windows 原生标题栏绘制，改为由
        // `gpui_component::TitleBar`（见 ui/mod.rs）自行绘制沉浸式标题栏，
        // 使其背景色与下方内容区域一致（默认跟随主题 `background` 色）。
        // 注意：`title` 字段仍需保留实际窗口标题文本（不能设为 `None`），
        // 因为它同时是任务栏/Alt+Tab 显示的窗口名，也是 `single_instance.rs`
        // 中 `FindWindowW` 用来查找已运行实例窗口的匹配依据。
        let window_options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from(paths::APP_NAME)),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.0), px(9.0))),
            }),
            window_bounds: Some(WindowBounds::centered(size(px(1200.), px(800.)), cx)),
            ..Default::default()
        };

        let view_holder: Rc<RefCell<Option<Entity<WallpaperLibrary>>>> =
            Rc::new(RefCell::new(None));
        let view_holder_for_window = view_holder.clone();

        let window = cx
            .open_window(window_options, move |window, cx| {
                let view = cx.new(|cx| WallpaperLibrary::new(window, cx));
                *view_holder_for_window.borrow_mut() = Some(view.clone());

                // 主题：默认浅色（白天），启动时以及此后系统主题变化时都自动跟随系统
                // 深色/浅色模式切换（见 AGENTS.md「主题」一节）。
                Theme::sync_system_appearance(Some(window), cx);
                window
                    .observe_window_appearance(|window, cx| {
                        Theme::sync_system_appearance(Some(window), cx);
                    })
                    .detach();

                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("创建主窗口失败");

        let view = view_holder
            .borrow()
            .clone()
            .expect("主视图应已在打开窗口时创建");

        // 应用退出时，尝试通过 RPC 优雅关闭内置的 aria2c.exe 常驻进程
        // （若从未启动过下载，aria2_handle 内部为 None，则什么也不做）。
        cx.on_app_quit({
            let view = view.clone();
            move |cx| {
                let aria2_handle = view.read(cx).aria2_handle();
                async move {
                    let manager = aria2_handle.borrow().clone();
                    if let Some(manager) = manager {
                        manager.shutdown().await;
                    }
                }
            }
        })
        .detach();

        let view_for_update = view.clone();
        cx.spawn(async move |cx| {
            // 优先加载本地缓存，尽快展示已知内容。
            if let Ok(Some(cached)) = fetcher::load_cache() {
                let _ = window.update(cx, |_, _, app_cx| {
                    view.update(app_cx, |this, cx| {
                        this.set_entries(cached, cx);
                    });
                });
            }

            refresh_once(&view, &window, cx).await;

            loop {
                cx.background_executor().timer(REFRESH_INTERVAL).await;
                refresh_once(&view, &window, cx).await;
            }
        })
        .detach();

        // 启动数秒后静默检查一次 GitHub 上是否有新版本发布，避免与首屏壁纸列表
        // 加载抢占带宽/注意力；发现新版本时弹出对话框，未发现或检查失败时静默。
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            check_update_once(&view_for_update, cx).await;
        })
        .detach();
    });
}

/// 检查一次更新并在发现新版本时弹出对话框。
///
/// 注意：这里必须通过 `view.downgrade().update_in(cx, ...)` 直接更新
/// `WallpaperLibrary` 这一个实体，而**不能**先用 `window.update(cx, |_, window, app_cx| { view.update(app_cx, ...) })`
/// 包一层——`window.update` 本身会先对 `Root` 实体加锁，而 `open_update_dialog`
/// 内部又会调用 `window.open_dialog`，后者同样需要对 `Root` 加锁，两次嵌套加锁
/// 同一个实体会触发 GPUI 的 `cannot update Root while it is already being updated`
/// panic（release 构建下 `panic = "abort"`，表现为静默闪退）。`update_in` 通过
/// `WeakEntity` 直接定位并更新 `WallpaperLibrary` 所在的窗口，不会触碰 `Root`
/// 的更新锁，因此不会与 `open_dialog` 内部的加锁冲突。
async fn check_update_once(view: &Entity<WallpaperLibrary>, cx: &mut AsyncApp) {
    let http = cx.update(|app| app.http_client());
    match updater::check_for_update(http).await {
        Ok(Some(release)) => {
            let _ = view.downgrade().update_in(cx, |this, window, cx| {
                this.open_update_dialog(release, window, cx);
            });
        }
        Ok(None) => {}
        Err(err) => {
            log::warn!("检查更新失败: {err}");
        }
    }
}

async fn refresh_once(
    view: &Entity<WallpaperLibrary>,
    window: &WindowHandle<Root>,
    cx: &mut AsyncApp,
) {
    let http = cx.update(|app| app.http_client());
    match fetcher::fetch_all(http).await {
        Ok(entries) => {
            let is_new = fetcher::load_cache()
                .ok()
                .flatten()
                .map(|cached| fetcher::has_new_entry(&cached, &entries))
                .unwrap_or(true);
            let _ = fetcher::save_cache(&entries);
            let _ = window.update(cx, |_, _, app_cx| {
                view.update(app_cx, |this, cx| {
                    this.set_entries(entries, cx);
                    if is_new {
                        this.set_status("检测到新的一天壁纸，已自动更新", cx);
                    }
                });
            });
        }
        Err(err) => {
            let _ = window.update(cx, |_, _, app_cx| {
                view.update(app_cx, |this, cx| {
                    this.set_status(format!("获取壁纸列表失败: {err}"), cx);
                });
            });
        }
    }
}
