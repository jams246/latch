mod cursor;
mod paint;
#[cfg(test)]
mod tests;

use cursor::Grip;
use paint::{Paint, pixels};
use std::cell::RefCell;
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::InvalidateRect,
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    core::w,
};

const SUBCLASS: usize = 0x4c415450;
const STYLES: WINDOW_EX_STYLE = WINDOW_EX_STYLE(
    WS_EX_LAYERED.0
        | WS_EX_TRANSPARENT.0
        | WS_EX_NOACTIVATE.0
        | WS_EX_TOOLWINDOW.0
        | WS_EX_TOPMOST.0,
);

pub(super) struct Preview {
    hwnd: HWND,
    state: Box<RefCell<Paint>>,
    _grip: Grip,
}

impl Preview {
    pub(super) fn new(owner: HWND, name: &str) -> anyhow::Result<Self> {
        let grip = Grip::new(owner)?;
        let state = Box::new(RefCell::new(Paint {
            name: name.encode_utf16().collect(),
            allowed: false,
            dpi: 96,
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                STYLES,
                w!("STATIC"),
                w!("Latch tab drag preview"),
                WS_POPUP | WS_DISABLED,
                0,
                0,
                0,
                0,
                Some(owner),
                None,
                None,
                None,
            )?
        };
        let preview = Self {
            hwnd,
            state,
            _grip: grip,
        };
        unsafe {
            SetWindowSubclass(
                hwnd,
                Some(window_proc),
                SUBCLASS,
                (&*preview.state as *const RefCell<Paint>) as usize,
            )
            .ok()?;
            SetLayeredWindowAttributes(hwnd, COLORREF(0xff00ff), 200, LWA_ALPHA | LWA_COLORKEY)?;
        }
        Ok(preview)
    }

    pub(super) fn update(&mut self, allowed: bool) -> anyhow::Result<()> {
        let mut pointer = Default::default();
        unsafe {
            GetCursorPos(&mut pointer)?;
        }
        self.position(pointer, allowed)
    }

    fn position(
        &mut self,
        pointer: windows::Win32::Foundation::POINT,
        allowed: bool,
    ) -> anyhow::Result<()> {
        let dpi = unsafe { GetDpiForWindow(self.hwnd) }.max(96);
        let changed = {
            let mut state = self.state.borrow_mut();
            let changed = state.allowed != allowed || state.dpi != dpi;
            state.allowed = allowed;
            state.dpi = dpi;
            changed
        };
        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                pointer.x.saturating_add(pixels(16, dpi)),
                pointer.y.saturating_add(pixels(18, dpi)),
                pixels(230, dpi),
                pixels(38, dpi),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
            if changed {
                let _ = InvalidateRect(Some(self.hwnd), None, false);
            }
        }
        Ok(())
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        // Remove the callback before its boxed paint state is released.
        unsafe {
            let _ = RemoveWindowSubclass(self.hwnd, Some(window_proc), SUBCLASS);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    state: usize,
) -> LRESULT {
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            // The UI-thread preview owns this stable allocation until the subclass is removed.
            let state = unsafe { &*(state as *const RefCell<Paint>) };
            paint::draw(hwnd, &mut state.borrow_mut());
            LRESULT(0)
        }
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}
