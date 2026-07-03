//! Windows 开机自启（HKCU\Software\Microsoft\Windows\CurrentVersion\Run）。

use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "BingWallpaperLib";

fn to_wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value
        .as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn open_run_key(access: REG_SAM_FLAGS) -> Result<HKEY> {
    let mut key = HKEY::default();
    let subkey = to_wide(RUN_KEY);
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            access,
            &mut key,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(key)
    } else {
        Err(windows::core::Error::from_win32()).context("打开开机自启注册表项失败")
    }
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let key = open_run_key(KEY_SET_VALUE)?;
    let name = to_wide(VALUE_NAME);
    let result = if enabled {
        let exe = std::env::current_exe().context("获取当前程序路径失败")?;
        let command = format!("\"{}\" --background", exe.display());
        let command_wide = to_wide(command);
        let bytes = unsafe {
            std::slice::from_raw_parts(
                command_wide.as_ptr().cast::<u8>(),
                command_wide.len() * std::mem::size_of::<u16>(),
            )
        };
        let status = unsafe { RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(bytes)) };
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32()).context("写入开机自启注册表失败")
        }
    } else {
        let status = unsafe { RegDeleteValueW(key, PCWSTR(name.as_ptr())) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(windows::core::Error::from_win32()).context("删除开机自启注册表失败")
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    result
}

pub fn is_enabled() -> bool {
    open_run_key(KEY_QUERY_VALUE)
        .map(|key| {
            let name = to_wide(VALUE_NAME);
            let status = unsafe {
                windows::Win32::System::Registry::RegQueryValueExW(
                    key,
                    PCWSTR(name.as_ptr()),
                    None,
                    None,
                    None,
                    None,
                )
            };
            unsafe {
                let _ = RegCloseKey(key);
            }
            status == ERROR_SUCCESS
        })
        .unwrap_or(false)
}
