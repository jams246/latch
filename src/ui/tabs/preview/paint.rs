use crate::ui::theme::CARBON;
use windows::{
    Win32::{
        Foundation::{COLORREF, HWND, RECT},
        Graphics::Gdi::*,
    },
    core::w,
};

pub(super) struct Paint {
    pub(super) name: Vec<u16>,
    pub(super) allowed: bool,
    pub(super) dpi: u32,
}

pub(super) fn pixels(value: i32, dpi: u32) -> i32 {
    (i64::from(value) * i64::from(dpi) / 96) as i32
}

fn color(rgb: u32) -> COLORREF {
    COLORREF(((rgb & 255) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 255))
}

struct Object(HGDIOBJ);
impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.0);
        }
    }
}

pub(super) fn draw(hwnd: HWND, state: &mut Paint) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut paint);
        if !dc.is_invalid() {
            let saved = SaveDC(dc);
            let mut bounds = RECT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut bounds);
            let clear = Object(CreateSolidBrush(COLORREF(0xff00ff)).into());
            FillRect(dc, &bounds, HBRUSH(clear.0.0));
            let background = Object(CreateSolidBrush(color(CARBON.selected)).into());
            let accent = Object(
                CreatePen(PS_SOLID, pixels(1, state.dpi).max(1), color(CARBON.accent)).into(),
            );
            SelectObject(dc, background.0);
            SelectObject(dc, accent.0);
            let _ = RoundRect(
                dc,
                0,
                0,
                bounds.right,
                bounds.bottom,
                pixels(10, state.dpi),
                pixels(10, state.dpi),
            );
            let font = Object(
                CreateFontW(
                    -pixels(14, state.dpi),
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    DEFAULT_PITCH.0 as u32,
                    w!("Segoe UI"),
                )
                .into(),
            );
            if !font.0.is_invalid() {
                SelectObject(dc, font.0);
            }
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(dc, color(CARBON.text));
            let mut text = RECT {
                left: pixels(12, state.dpi),
                top: 0,
                right: bounds.right - pixels(if state.allowed { 12 } else { 38 }, state.dpi),
                bottom: bounds.bottom,
            };
            DrawTextW(
                dc,
                &mut state.name,
                &mut text,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
            if !state.allowed {
                let danger = Object(
                    CreatePen(PS_SOLID, pixels(2, state.dpi).max(1), color(CARBON.danger)).into(),
                );
                SelectObject(dc, danger.0);
                let left = bounds.right - pixels(28, state.dpi);
                let top = pixels(11, state.dpi);
                let edge = pixels(16, state.dpi);
                let _ = Ellipse(dc, left, top, left + edge, top + edge);
                let inset = pixels(3, state.dpi);
                let _ = MoveToEx(dc, left + inset, top + inset, None);
                let _ = LineTo(dc, left + edge - inset, top + edge - inset);
                SelectObject(dc, accent.0);
            }
            let _ = RestoreDC(dc, saved);
        }
        let _ = EndPaint(hwnd, &paint);
    }
}
