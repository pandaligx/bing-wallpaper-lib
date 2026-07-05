//! 主窗口尺寸与位置计算。
//!
//! 默认窗口仍以 1200x800 为目标，但会按当前 Windows 工作区（扣除任务栏）
//! 自动缩小并居中，避免 1366x768、800x600 等小分辨率下窗口上下边框超出屏幕。

use gpui::{point, px, size, Bounds, Pixels};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
};

pub const DEFAULT_WINDOW_WIDTH: i32 = 1200;
pub const DEFAULT_WINDOW_HEIGHT: i32 = 800;
const MIN_WINDOW_WIDTH: i32 = 640;
const MIN_WINDOW_HEIGHT: i32 = 480;
const LARGE_SCREEN_MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub fn default_window_bounds() -> Bounds<Pixels> {
    let placement = default_window_placement();
    Bounds::new(
        point(px(placement.x as f32), px(placement.y as f32)),
        size(px(placement.width as f32), px(placement.height as f32)),
    )
}

pub fn default_window_placement() -> WindowPlacement {
    let work_area = windows_work_area().unwrap_or(WorkArea {
        left: 0,
        top: 0,
        right: DEFAULT_WINDOW_WIDTH,
        bottom: DEFAULT_WINDOW_HEIGHT,
    });

    fit_window_in_work_area(work_area)
}

fn fit_window_in_work_area(work_area: WorkArea) -> WindowPlacement {
    let work_width = (work_area.right - work_area.left).max(1);
    let work_height = (work_area.bottom - work_area.top).max(1);
    let min_width = MIN_WINDOW_WIDTH.min(work_width);
    let min_height = MIN_WINDOW_HEIGHT.min(work_height);
    let horizontal_margin = if work_width > DEFAULT_WINDOW_WIDTH + LARGE_SCREEN_MARGIN * 2 {
        LARGE_SCREEN_MARGIN
    } else {
        0
    };
    let vertical_margin = if work_height > DEFAULT_WINDOW_HEIGHT + LARGE_SCREEN_MARGIN * 2 {
        LARGE_SCREEN_MARGIN
    } else {
        0
    };

    let available_width = (work_width - horizontal_margin * 2).max(min_width);
    let available_height = (work_height - vertical_margin * 2).max(min_height);
    let width = DEFAULT_WINDOW_WIDTH.min(available_width).max(min_width);
    let height = DEFAULT_WINDOW_HEIGHT.min(available_height).max(min_height);
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
    fn fits_default_window_inside_1366_by_768_work_area() {
        let placement = fit_window_in_work_area(WorkArea {
            left: 0,
            top: 0,
            right: 1366,
            bottom: 728,
        });

        assert_eq!(placement.width, 1200);
        assert_eq!(placement.height, 728);
        assert!(placement.y >= 0);
        assert!(placement.y + placement.height <= 728);
    }

    #[test]
    fn fits_window_inside_800_by_600_work_area() {
        let placement = fit_window_in_work_area(WorkArea {
            left: 0,
            top: 0,
            right: 800,
            bottom: 560,
        });

        assert!(placement.width <= 800);
        assert!(placement.height <= 560);
        assert!(placement.x >= 0);
        assert!(placement.y >= 0);
        assert!(placement.x + placement.width <= 800);
        assert!(placement.y + placement.height <= 560);
    }

    #[test]
    fn never_exceeds_tiny_work_area() {
        let placement = fit_window_in_work_area(WorkArea {
            left: 0,
            top: 0,
            right: 500,
            bottom: 360,
        });

        assert_eq!(placement.width, 500);
        assert_eq!(placement.height, 360);
        assert_eq!(placement.x, 0);
        assert_eq!(placement.y, 0);
    }
}
