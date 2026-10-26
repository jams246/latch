use gpui::{Bounds, Pixels, Point, Window, point, px, size};
use latch::storage::WindowSize;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(not(test))]
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, IsWindowEnabled, VK_LBUTTON},
        WindowsAndMessaging::{GA_ROOT, GetAncestor, GetCursorPos, WindowFromPoint},
    },
};

pub(in crate::ui) fn hwnd(window: &Window) -> anyhow::Result<HWND> {
    match HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Cannot access native window: {error}"))?
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => anyhow::bail!("Latch requires native Windows windows"),
    }
}

pub(super) fn captured(window: &Window) -> bool {
    hwnd(window).is_ok_and(|handle| unsafe {
        GetCapture() == handle && GetAsyncKeyState(i32::from(VK_LBUTTON.0)) < 0
    })
}

pub(super) fn pointer_in(window: &Window) -> Option<Point<Pixels>> {
    let handle = hwnd(window).ok()?;
    let mut pointer = POINT::default();
    // Native screen coordinates avoid mixing the source and destination DPI scales.
    unsafe {
        GetCursorPos(&mut pointer).ok()?;
        if GetAncestor(WindowFromPoint(pointer), GA_ROOT) != handle
            || !IsWindowEnabled(handle).as_bool()
        {
            return None;
        }
        ScreenToClient(handle, &mut pointer).ok().ok()?;
    }
    let scale = window.scale_factor();
    Some(point(
        px(pointer.x as f32 / scale),
        px(pointer.y as f32 / scale),
    ))
}

#[cfg(not(test))]
pub(in crate::ui) fn placement(
    window: &Window,
    saved: WindowSize,
) -> anyhow::Result<Bounds<Pixels>> {
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        GetMonitorInfoW(
            MonitorFromWindow(hwnd(window)?, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        )
        .ok()?;
    }
    let scale = window.scale_factor();
    let work = monitor.rcWork;
    let left = work.left as f32 / scale;
    let top = work.top as f32 / scale;
    let width = saved
        .width
        .min((work.right - work.left) as f32 / scale)
        .max(WindowSize::MIN_WIDTH);
    let height = saved
        .height
        .min((work.bottom - work.top) as f32 / scale)
        .max(WindowSize::MIN_HEIGHT);
    let source = window.bounds().origin;
    let x = (f32::from(source.x) + 32.).clamp(left, (work.right as f32 / scale - width).max(left));
    let y = (f32::from(source.y) + 32.).clamp(top, (work.bottom as f32 / scale - height).max(top));
    Ok(Bounds::new(
        point(px(x), px(y)),
        size(px(width), px(height)),
    ))
}

#[cfg(test)]
pub(in crate::ui) fn placement(
    window: &Window,
    saved: WindowSize,
) -> anyhow::Result<Bounds<Pixels>> {
    Ok(Bounds::new(
        window.bounds().origin + point(px(32.), px(32.)),
        size(px(saved.width), px(saved.height)),
    ))
}
