//! Windows 系统托盘图标与右键菜单。
//!
//! GPUI 当前版本没有公开的系统托盘 API，因此这里直接使用 Windows Shell_NotifyIconW。
//! 托盘窗口在独立线程中运行，只通过 channel 把用户点击的菜单命令传回 GPUI 主任务处理。

use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;
use std::thread;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, CS_HREDRAW, CS_VREDRAW, HMENU, MF_CHECKED, MF_SEPARATOR,
    MF_STRING, MF_UNCHECKED, MSG, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_USER, WNDCLASSW,
};

const TRAY_ID: u32 = 1;
const WM_TRAY: u32 = WM_USER + 42;
const ID_SHOW: usize = 1001;
const ID_TOGGLE_STARTUP: usize = 1002;
const ID_TOGGLE_RESIDENT: usize = 1003;
const ID_TOGGLE_AUTO: usize = 1004;
const ID_CHANGE_NOW: usize = 1005;
const ID_EXIT: usize = 1099;
const CLASS_NAME: &str = "BingWallpaperLibTrayWindow";

static COMMAND_SENDER: OnceLock<Sender<TrayCommand>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub enum TrayCommand {
    ShowWindow,
    ToggleStartup,
    ToggleResident,
    ToggleAutoWallpaper,
    ChangeWallpaperNow,
    Quit,
}

fn to_wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn fill_wide<const N: usize>(target: &mut [u16; N], text: &str) {
    let wide = to_wide(text);
    let len = wide.len().min(N);
    target[..len].copy_from_slice(&wide[..len]);
}

fn send(command: TrayCommand) {
    if let Some(sender) = COMMAND_SENDER.get() {
        let _ = sender.send(command);
    }
}

fn menu_text(text: &str) -> Vec<u16> {
    to_wide(text)
}

unsafe fn append_menu_item(menu: HMENU, id: usize, text: &str, checked: bool) {
    let text = menu_text(text);
    let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
    let _ = AppendMenuW(menu, flags, id, PCWSTR(text.as_ptr()));
}

unsafe fn show_context_menu(hwnd: HWND) {
    let menu = match CreatePopupMenu() {
        Ok(menu) => menu,
        Err(_) => return,
    };

    let settings = crate::settings::AppSettings::load();
    append_menu_item(menu, ID_SHOW, "打开主窗口", false);
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    append_menu_item(
        menu,
        ID_TOGGLE_STARTUP,
        "开机自启",
        settings.startup_enabled,
    );
    append_menu_item(
        menu,
        ID_TOGGLE_RESIDENT,
        "后台常驻",
        settings.background_resident_enabled,
    );
    append_menu_item(
        menu,
        ID_TOGGLE_AUTO,
        "每日自动壁纸",
        settings.auto_wallpaper_enabled,
    );
    append_menu_item(menu, ID_CHANGE_NOW, "按自动壁纸来源立即更换", false);
    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
    append_menu_item(menu, ID_EXIT, "退出", false);

    let mut point = POINT::default();
    if GetCursorPos(&mut point).is_ok() {
        let _ = SetForegroundWindow(hwnd);
        let id = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            None,
        );
        match id.0 as usize {
            ID_SHOW => send(TrayCommand::ShowWindow),
            ID_TOGGLE_STARTUP => send(TrayCommand::ToggleStartup),
            ID_TOGGLE_RESIDENT => send(TrayCommand::ToggleResident),
            ID_TOGGLE_AUTO => send(TrayCommand::ToggleAutoWallpaper),
            ID_CHANGE_NOW => send(TrayCommand::ChangeWallpaperNow),
            ID_EXIT => send(TrayCommand::Quit),
            _ => {}
        }
    }
    let _ = DestroyMenu(menu);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAY => match (lparam.0 as u32) & 0xffff {
            WM_LBUTTONDBLCLK | WM_LBUTTONUP | NIN_SELECT => {
                send(TrayCommand::ShowWindow);
                LRESULT(0)
            }
            WM_RBUTTONDOWN | WM_RBUTTONUP | WM_CONTEXTMENU => {
                show_context_menu(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        },
        WM_COMMAND => {
            match wparam.0 & 0xffff {
                ID_SHOW => send(TrayCommand::ShowWindow),
                ID_TOGGLE_STARTUP => send(TrayCommand::ToggleStartup),
                ID_TOGGLE_RESIDENT => send(TrayCommand::ToggleResident),
                ID_TOGGLE_AUTO => send(TrayCommand::ToggleAutoWallpaper),
                ID_CHANGE_NOW => send(TrayCommand::ChangeWallpaperNow),
                ID_EXIT => send(TrayCommand::Quit),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[allow(clippy::manual_dangling_ptr)]
unsafe fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage: WM_TRAY,
        ..Default::default()
    };
    if let Ok(module) = GetModuleHandleW(PCWSTR::null()) {
        if let Ok(icon) = LoadIconW(module, PCWSTR(1 as *const u16)) {
            data.hIcon = icon;
        }
    }
    fill_wide(&mut data.szTip, crate::paths::APP_NAME);
    data
}

unsafe fn add_tray_icon(hwnd: HWND) {
    let mut data = tray_data(hwnd);
    let _ = Shell_NotifyIconW(NIM_ADD, &data);
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    let _ = Shell_NotifyIconW(NIM_SETVERSION, &data);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
}

pub fn start() -> Result<Receiver<TrayCommand>> {
    let (tx, rx) = mpsc::channel();
    let _ = COMMAND_SENDER.set(tx);

    thread::Builder::new()
        .name("bing-wallpaper-tray".to_string())
        .spawn(move || unsafe {
            let class_name = to_wide(CLASS_NAME);
            let title = to_wide(crate::paths::app_window_title());
            let hinstance = match GetModuleHandleW(PCWSTR::null()) {
                Ok(module) => module,
                Err(err) => {
                    log::warn!("获取模块句柄失败，无法创建托盘窗口: {err}");
                    return;
                }
            };

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);

            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                HWND::default(),
                HMENU::default(),
                hinstance,
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(err) => {
                    log::warn!("创建托盘隐藏窗口失败: {err}");
                    return;
                }
            };

            add_tray_icon(hwnd);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }
            remove_tray_icon(hwnd);
        })
        .context("启动托盘线程失败")?;

    Ok(rx)
}
