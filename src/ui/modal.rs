//! Native owner association and balanced modality for authentication windows.

use gpui::{AnyWindowHandle, App, Window};

#[cfg(windows)]
use {
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    std::{cell::RefCell, collections::HashMap},
    windows::Win32::{
        Foundation::{GetLastError, HWND, SetLastError, WIN32_ERROR},
        UI::{
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::{GWLP_HWNDPARENT, IsWindow, SetWindowLongPtrW},
        },
    },
};

#[cfg(windows)]
thread_local! {
    static OWNERS: RefCell<HashMap<isize, OwnerState>> = RefCell::new(HashMap::new());
}

#[cfg(windows)]
struct OwnerState {
    dialogs: usize,
    restore_enabled: bool,
}

/// Keeps an authentication window's owner disabled until its last dialog closes.
pub(super) struct ModalGuard {
    #[cfg(windows)]
    owner: HWND,
}

/// Associate a native dialog with its main window and disable that owner.
pub(super) fn attach(
    dialog: &Window,
    owner: AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<ModalGuard> {
    #[cfg(windows)]
    {
        let owner = owner.update(cx, |_, window, _| hwnd(window))??;
        attach_windows(hwnd(dialog)?, owner)
    }
    #[cfg(not(windows))]
    {
        let _ = (dialog, owner, cx);
        anyhow::bail!("Native authentication window modality is unsupported on this platform")
    }
}

#[cfg(windows)]
fn hwnd(window: &Window) -> anyhow::Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("Cannot access native authentication window: {error}"))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut _)),
        _ => anyhow::bail!("Authentication requires a native Windows window"),
    }
}

#[cfg(windows)]
fn attach_windows(dialog: HWND, owner: HWND) -> anyhow::Result<ModalGuard> {
    // Both handles belong to this UI thread; the guard never leaves that thread.
    unsafe {
        anyhow::ensure!(
            IsWindow(Some(dialog)).as_bool() && IsWindow(Some(owner)).as_bool(),
            "Cannot associate a closed authentication window"
        );
        SetLastError(WIN32_ERROR(0));
        let previous = SetWindowLongPtrW(dialog, GWLP_HWNDPARENT, owner.0 as isize);
        let error = GetLastError();
        anyhow::ensure!(
            previous != 0 || error.0 == 0,
            "Cannot associate authentication window: Windows error {}",
            error.0
        );
        let was_enabled = IsWindowEnabled(owner).as_bool();
        let first = OWNERS.with(|owners| {
            let mut owners = owners.borrow_mut();
            let state = owners.entry(owner.0 as isize).or_insert(OwnerState {
                dialogs: 0,
                restore_enabled: was_enabled,
            });
            state.dialogs += 1;
            state.dialogs == 1
        });
        let guard = ModalGuard { owner };
        if first && was_enabled {
            let _ = EnableWindow(owner, false);
            anyhow::ensure!(
                !IsWindowEnabled(owner).as_bool(),
                "Cannot make authentication window modal"
            );
        }
        Ok(guard)
    }
}

impl Drop for ModalGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let restore = OWNERS.with(|owners| {
                let mut owners = owners.borrow_mut();
                let Some(state) = owners.get_mut(&(self.owner.0 as isize)) else {
                    return false;
                };
                state.dialogs -= 1;
                if state.dialogs != 0 {
                    return false;
                }
                owners
                    .remove(&(self.owner.0 as isize))
                    .is_some_and(|state| state.restore_enabled)
            });
            // The main window can already be destroyed during application shutdown.
            if restore && unsafe { IsWindow(Some(self.owner)).as_bool() } {
                let _ = unsafe { EnableWindow(self.owner, true) };
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows::{Win32::UI::WindowsAndMessaging::*, core::w};

    struct TestWindow(HWND);

    impl TestWindow {
        fn new() -> Self {
            // Predefined STATIC windows stay hidden and contain no user data.
            Self(unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Latch modality verification"),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    100,
                    100,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
            })
        }
    }

    impl Drop for TestWindow {
        fn drop(&mut self) {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn modality_should_keep_owner_disabled_until_last_dialog_closes() {
        let owner = TestWindow::new();
        let first_dialog = TestWindow::new();
        let second_dialog = TestWindow::new();
        let first = attach_windows(first_dialog.0, owner.0).unwrap();
        let second = attach_windows(second_dialog.0, owner.0).unwrap();
        assert_eq!(
            unsafe { GetWindow(first_dialog.0, GW_OWNER).unwrap() },
            owner.0
        );
        drop(first);
        assert!(!unsafe { IsWindowEnabled(owner.0).as_bool() });
        drop(second);
        assert!(unsafe { IsWindowEnabled(owner.0).as_bool() });
    }

    #[test]
    fn closed_window_should_fail_without_disabling_owner() {
        let owner = TestWindow::new();
        assert!(attach_windows(HWND::default(), owner.0).is_err());
    }

    #[test]
    fn closing_dialog_should_preserve_previously_disabled_owner() {
        let owner = TestWindow::new();
        let dialog = TestWindow::new();
        let _ = unsafe { EnableWindow(owner.0, false) };
        let guard = attach_windows(dialog.0, owner.0).unwrap();
        drop(guard);
        assert!(!unsafe { IsWindowEnabled(owner.0).as_bool() });
    }
}
