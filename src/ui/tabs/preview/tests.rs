use super::*;
use windows::Win32::{
    Foundation::POINT,
    UI::{Shell::GetWindowSubclass, WindowsAndMessaging::*},
};

#[test]
fn preview_is_nonactivating_click_through_owned_and_removed_on_drop() {
    unsafe {
        let owner = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Synthetic preview owner"),
            WS_POPUP,
            -30000,
            -30000,
            200,
            100,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let foreground = GetForegroundWindow();
        let hwnd = {
            let mut preview = Preview::new(owner, "Synthetic terminal").unwrap();
            let mut reference = 0;
            assert!(
                GetWindowSubclass(
                    owner,
                    Some(cursor::cursor_proc),
                    cursor::SUBCLASS,
                    Some(&mut reference)
                )
                .as_bool()
            );
            let mut icon = ICONINFO::default();
            GetIconInfo(HICON(reference as *mut _), &mut icon).unwrap();
            assert!(!icon.fIcon.as_bool());
            assert_eq!((icon.xHotspot, icon.yHotspot), (12, 9));
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(icon.hbmMask.into());
            if !icon.hbmColor.is_invalid() {
                let _ = windows::Win32::Graphics::Gdi::DeleteObject(icon.hbmColor.into());
            }
            let style = WINDOW_EX_STYLE(GetWindowLongPtrW(preview.hwnd, GWL_EXSTYLE) as u32);
            assert_eq!(style & STYLES, STYLES);
            assert_ne!(
                GetWindowLongPtrW(preview.hwnd, GWL_STYLE) as u32 & WS_DISABLED.0,
                0
            );
            assert_eq!(GetWindow(preview.hwnd, GW_OWNER).unwrap(), owner);
            assert_eq!(
                SendMessageW(preview.hwnd, WM_NCHITTEST, None, None).0,
                HTTRANSPARENT as isize
            );
            assert_eq!(
                SendMessageW(preview.hwnd, WM_MOUSEACTIVATE, None, None).0,
                MA_NOACTIVATE as isize
            );
            preview
                .position(
                    POINT {
                        x: -30000,
                        y: -30000,
                    },
                    true,
                )
                .unwrap();
            assert!(preview.state.borrow().allowed);
            assert_eq!(GetForegroundWindow(), foreground);
            preview
                .position(
                    POINT {
                        x: -29900,
                        y: -29900,
                    },
                    false,
                )
                .unwrap();
            assert!(!preview.state.borrow().allowed);
            preview.hwnd
        };
        assert!(!IsWindow(Some(hwnd)).as_bool());
        let mut reference = 0;
        assert!(
            !GetWindowSubclass(
                owner,
                Some(cursor::cursor_proc),
                cursor::SUBCLASS,
                Some(&mut reference)
            )
            .as_bool()
        );
        DestroyWindow(owner).unwrap();
    }
}

#[test]
fn native_preview_rejects_invalid_owner_without_panicking() {
    assert!(Preview::new(HWND::default(), "Synthetic terminal").is_err());
}

#[test]
fn preview_dimensions_scale_in_physical_pixels() {
    assert_eq!(pixels(230, 96), 230);
    assert_eq!(pixels(230, 144), 345);
    assert_eq!(pixels(38, 192), 76);
}

#[test]
#[ignore = "Interactive native preview verification fixture"]
fn gui_preview() {
    use std::{path::PathBuf, time::Duration};
    unsafe {
        let ready = PathBuf::from(
            std::env::var_os("LATCH_PREVIEW_FIXTURE_READY").expect("Set fixture readiness path"),
        );
        assert!(ready.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".verification")));
        let allowed = std::env::var_os("LATCH_PREVIEW_ALLOWED").is_some();
        let owner = CreateWindowExW(
            WS_EX_NOACTIVATE,
            w!("STATIC"),
            w!("Latch drag preview verification"),
            WS_OVERLAPPEDWINDOW,
            200,
            150,
            600,
            250,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // The first call can inherit SW_HIDE from the test process startup options.
        let _ = ShowWindow(owner, SW_SHOWNOACTIVATE);
        let _ = ShowWindow(owner, SW_SHOWNOACTIVATE);
        let foreground = GetForegroundWindow();
        let mut preview = Preview::new(owner, "Synthetic server · SFTP").unwrap();
        preview.position(POINT { x: 360, y: 250 }, allowed).unwrap();
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(preview.hwnd);
        assert_ne!(WindowFromPoint(POINT { x: 400, y: 280 }), preview.hwnd);
        std::fs::write(
            ready,
            serde_json::to_vec(&serde_json::json!({
                "owner": owner.0 as usize, "preview": preview.hwnd.0 as usize, "allowed": allowed,
                "pid": std::process::id(),
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(GetForegroundWindow(), foreground);
        loop {
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

#[test]
fn closing_owner_before_preview_drop_is_safe() {
    unsafe {
        let owner = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("Synthetic preview owner"),
            WS_POPUP,
            -30000,
            -30000,
            200,
            100,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let preview = Preview::new(owner, "Synthetic terminal").unwrap();
        DestroyWindow(owner).unwrap();
        assert!(!IsWindow(Some(preview.hwnd)).as_bool());
        drop(preview);
    }
}
