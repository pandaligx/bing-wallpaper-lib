//! 管理员权限自动提权。
//!
//! 由于我们没有为最终可执行文件嵌入 `requireAdministrator` 的 Windows 清单
//! （这会与 gpui 自身内嵌的 `asInvoker` 清单资源冲突，导致链接期资源重复），
//! 因此改为在程序启动的最早阶段，于运行时检测当前进程是否已提权；
//! 如果不是管理员权限，则通过 `ShellExecuteW` 以 `runas` 谓词重新启动自身
//! （此时系统会弹出 UAC 提示框），随后立即退出当前的非提权进程。

use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// 判断当前进程是否已经以管理员身份运行。
fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned_len = 0u32;
        let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            size,
            &mut returned_len,
        );
        let _ = CloseHandle(token);

        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 以管理员权限重新启动当前可执行文件。
fn relaunch_as_admin() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe_wide = to_wide(&exe.to_string_lossy());

    // 将当前命令行参数（去掉程序名本身）透传给提权后的新进程。
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_joined = args
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(" ");
    let args_wide = to_wide(&args_joined);

    let verb = to_wide("runas");
    let dir_wide;
    let dir_ptr = match exe.parent() {
        Some(p) => {
            dir_wide = to_wide(&p.to_string_lossy());
            PCWSTR(dir_wide.as_ptr())
        }
        None => PCWSTR::null(),
    };

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            dir_ptr,
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW 返回值大于 32 表示成功。
    (result.0 as isize) > 32
}

/// 确保当前进程以管理员权限运行。
///
/// 如果尚未提权，则尝试拉起一个提权后的新进程并退出当前进程（返回 `false`
/// 表示调用方应立即结束 `main`，不再继续初始化 UI）；如果已经是管理员，
/// 或者提权失败（例如用户在 UAC 对话框中点击了“取消”），则返回 `true`，
/// 由调用方决定是否继续以普通权限运行。
pub fn ensure_elevated() -> bool {
    if is_elevated() {
        return true;
    }

    if relaunch_as_admin() {
        false
    } else {
        log::warn!("以管理员身份重新启动失败或被用户取消，将以当前权限继续运行");
        true
    }
}
