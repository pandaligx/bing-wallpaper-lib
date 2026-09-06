//! 主窗口尺寸与位置计算。
//!
//! 默认窗口仍以 1200x800 为目标，但会按当前 Windows 工作区（扣除任务栏）
//! 自动缩小并居中，避免 1366x768、800x600 等小分辨率下窗口上下边框超出屏幕。

use gpui::{point, px, size, Bounds, Pixels};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

pub const DEFAULT_WINDOW_WIDTH: i32 = 1200;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 800;
const LARGE_SCREEN_MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn default_window_bounds() -> Bounds<Pixels> {
    let scale = primary_scale_factor();
    let placement = placement_for_scale(scale);
    Bounds::new(
        point(
            px(placement.x as f32 / scale),
            px(placement.y as f32 / scale),
        ),
        size(
            px(placement.width as f32 / scale),
            px(placement.height as f32 / scale),
        ),
    )
}

pub fn default_window_placement() -> WindowPlacement {
    placement_for_scale(primary_scale_factor())
}

fn primary_scale_factor() -> f32 {
    let monitor = unsafe { MonitorFromPoint(POINT::default(), MONITOR_DEFAULTTOPRIMARY) };
    let (mut x, mut y) = (96, 96);
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.is_ok() && x > 0 {
        x as f32 / 96.0
    } else {
        1.0
    }
}

fn placement_for_scale(scale: f32) -> WindowPlacement {
    let work_area = windows_work_area().unwrap_or(WorkArea {
        left: 0,
        top: 0,
        right: DEFAULT_WINDOW_WIDTH,
        bottom: DEFAULT_WINDOW_HEIGHT,
    });

    fit_window_in_work_area(work_area, scale)
}

// Win32 placement uses physical pixels; GPUI bounds use logical pixels.
fn fit_window_in_work_area(work_area: WorkArea, scale: f32) -> WindowPlacement {
    let work_width = (work_area.right - work_area.left).max(1);
    let work_height = (work_area.bottom - work_area.top).max(1);
    let scaled = |value: i32| (value as f32 * scale).round() as i32;
    // Reserve space for native resize borders even when the default UI must shrink.
    let margin = scaled(LARGE_SCREEN_MARGIN);
    let width = scaled(DEFAULT_WINDOW_WIDTH).min((work_width - margin * 2).max(1));
    let height = scaled(DEFAULT_WINDOW_HEIGHT).min((work_height - margin * 2).max(1));
    let x = work_area.left + (work_width - width) / 2;
    let y = work_area.top + (work_height - height) / 2;

    WindowPlacement {
        x,
        y,
        width,
        height,
    }
}

#[derive(Debug, Clone, Copy)]
struct WorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn windows_work_area() -> Option<WorkArea> {
    let mut rect = RECT::default();
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };

    ok.ok()?;
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return None;
    }

    Some(WorkArea {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_and_centers_at_every_supported_scale() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0] {
            for (left, top, width, height) in [
                (0, 0, 1920, 1040),
                (0, 48, 1920, 1032),
                (-1920, -200, 1920, 1040),
                (0, 0, 1366, 728),
                (0, 0, 500, 360),
            ] {
                let p = fit_window_in_work_area(
                    WorkArea {
                        left,
                        top,
                        right: left + width,
                        bottom: top + height,
                    },
                    scale,
                );
                assert!(p.width > 0 && p.height > 0);
                assert!(p.x >= left && p.y >= top);
                assert!(p.x + p.width <= left + width);
                assert!(p.y + p.height <= top + height);
                assert!((2 * (p.x - left) + p.width - width).abs() <= 1);
                assert!((2 * (p.y - top) + p.height - height).abs() <= 1);
                assert!(p.width as f32 / scale <= DEFAULT_WINDOW_WIDTH as f32);
                assert!(p.height as f32 / scale <= DEFAULT_WINDOW_HEIGHT as f32);
            }
        }
    }
}
