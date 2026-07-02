//! Windows 原生文件夹选择对话框，用于让用户选择壁纸下载目录。

use anyhow::{Context, Result};
use std::path::PathBuf;

#[cfg(windows)]
pub fn pick_folder() -> Result<Option<PathBuf>> {
    use windows::core::{w, Error, HRESULT};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH,
    };

    const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106u32 as i32);
    const HRESULT_FROM_WIN32_CANCELLED: HRESULT = HRESULT(0x8007_04C7u32 as i32);

    // GPUI/Windows 平台层可能已经初始化过 COM；若初始化模式不同，继续使用当前线程已有的 COM 模式。
    let init_hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let initialized_com = if init_hr.is_ok() {
        true
    } else if init_hr == RPC_E_CHANGED_MODE {
        false
    } else {
        return Err(Error::from(init_hr)).context("初始化文件夹选择对话框失败");
    };

    let result = unsafe {
        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .context("创建文件夹选择对话框失败")?;
        dialog
            .SetTitle(w!("选择壁纸下载保存路径"))
            .context("设置文件夹选择对话框标题失败")?;
        dialog
            .SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .context("设置文件夹选择对话框选项失败")?;

        match dialog.Show(HWND(std::ptr::null_mut())) {
            Ok(()) => {
                let item = dialog.GetResult().context("读取所选文件夹失败")?;
                let display_name = item
                    .GetDisplayName(SIGDN_FILESYSPATH)
                    .context("读取所选文件夹路径失败")?;
                let path = display_name
                    .to_string()
                    .context("所选文件夹路径不是合法字符串")?;
                CoTaskMemFree(Some(display_name.as_ptr().cast()));
                Ok(Some(PathBuf::from(path)))
            }
            Err(err) if err.code() == HRESULT_FROM_WIN32_CANCELLED => Ok(None),
            Err(err) => Err(err).context("打开文件夹选择对话框失败"),
        }
    };

    if initialized_com {
        unsafe { CoUninitialize() };
    }

    result
}

#[cfg(not(windows))]
pub fn pick_folder() -> Result<Option<PathBuf>> {
    Ok(None)
}
