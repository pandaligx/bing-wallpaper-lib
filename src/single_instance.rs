//! 单实例检测：避免用户重复启动程序时，同时打开多个主窗口
//! （并各自独立启动/管理一份内置 `aria2c.exe` 子进程）。
//!
//! 实现方式：使用一个全局命名的 `Mutex` 内核对象。第一个启动的进程会成功创建它并
//! 一直持有到进程退出；后续再次启动的进程创建同名 Mutex 时会返回一个指向已存在对象
//! 的句柄，同时 `GetLastError()` 会返回 `ERROR_ALREADY_EXISTS`，据此判断"已有实例在
//! 运行"，此时尝试把已运行实例的窗口带到前台，然后让当前（新）进程退出。

use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// 全局命名互斥体名称。加上 `Global\` 前缀，确保在多用户会话场景下也能正确检测。
const MUTEX_NAME: &str = "Global\\BingWallpaperLib_SingleInstance";

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 确保当前进程是唯一实例。
///
/// 返回 `true` 表示当前进程可以继续正常启动；返回 `false` 表示检测到另一个实例已经
/// 在运行，调用方应立即结束 `main`（本函数已尝试将已运行实例的窗口带到前台）。
pub fn ensure_single_instance() -> bool {
    let name = to_wide(MUTEX_NAME);
    let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) };

    let already_running = match handle {
        Ok(_) => (unsafe { GetLastError() }) == ERROR_ALREADY_EXISTS,
        Err(_) => false,
    };

    // 故意"泄漏"这个句柄：需要让它一直存活到进程退出，由操作系统在进程终止时自动回收，
    // 不能提前 CloseHandle，否则会失去互斥体的持有，导致检测失效。
    std::mem::forget(handle);

    if already_running {
        activate_existing_window();
        return false;
    }
    true
}

/// 尝试找到已运行实例的主窗口并将其带到前台。
fn activate_existing_window() {
    let title = to_wide(&crate::paths::app_window_title());
    unsafe {
        if let Ok(hwnd) = FindWindowW(PCWSTR::null(), PCWSTR(title.as_ptr())) {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}
