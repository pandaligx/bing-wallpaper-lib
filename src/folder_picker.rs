//! Windows 原生文件夹选择对话框，用于让用户选择壁纸下载目录。

use anyhow::{Context, Result};
use std::path::PathBuf;

#[cfg(windows)]
pub fn pick_folder() -> Result<Option<PathBuf>> {
    let (sender, receiver) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("folder-picker-sta".to_string())
        .spawn(move || {
            let _ = sender.send(pick_folder_on_sta_thread());
        })
        .context("启动文件夹选择对话框线程失败")?;

    receiver.recv().context("文件夹选择对话框线程异常退出")?
}

#[cfg(windows)]
fn pick_folder_on_sta_thread() -> Result<Option<PathBuf>> {
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

    const HRESULT_FROM_WIN32_CANCELLED: HRESULT = HRESULT(0x8007_04C7u32 as i32);

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    // Windows Shell 的 IFileOpenDialog 应在 STA 线程中创建/显示。不要复用 GPUI
    // 主线程的 COM 环境，否则若主线程已是其他 apartment 模式，文件夹选择
    // 对话框可能随机卡死或导致进程闪退。
    let init_hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if init_hr.is_err() {
        return Err(Error::from(init_hr)).context("初始化文件夹选择对话框失败");
    }
    let _com = ComApartment;

    unsafe {
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
    }
}

#[cfg(not(windows))]
pub fn pick_folder() -> Result<Option<PathBuf>> {
    Ok(None)
}
