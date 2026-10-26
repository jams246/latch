use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, GetCapture, VK_LBUTTON},
        Shell::{DefSubclassProc, GetWindowSubclass, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

pub(super) const SUBCLASS: usize = 0x4c415443;
const HAND: [&str; 22] = [
    "       ##  ##  ##",
    "      #..##..##..##",
    "    ###..##..##..#.#",
    "   #..#..#..#..#...#",
    "   #..#..#..#..#...#",
    "   #..#.............#",
    "   #..#.............#",
    "   #..#.............#",
    "   #...............#",
    "   #...............#",
    "   #...............#",
    "    #.............#",
    "    #.............#",
    "     #...........#",
    "      #.........#",
    "      #.........#",
    "      #.........#",
    "      #.........#",
    "      #.........#",
    "      ###########",
    "",
    "",
];

pub(super) struct Grip {
    owner: HWND,
    cursor: HCURSOR,
    previous: Option<HCURSOR>,
}

impl Grip {
    pub(super) fn new(owner: HWND) -> anyhow::Result<Self> {
        let (and, xor) = masks();
        let cursor = unsafe {
            CreateCursor(
                None,
                12,
                9,
                32,
                32,
                and.as_ptr().cast(),
                xor.as_ptr().cast(),
            )?
        };
        let grip = Self {
            owner,
            cursor,
            previous: None,
        };
        unsafe {
            SetWindowSubclass(owner, Some(cursor_proc), SUBCLASS, cursor.0 as usize).ok()?;
        }
        let mut grip = grip;
        if holding(owner) {
            grip.previous = Some(unsafe { SetCursor(Some(cursor)) });
        }
        Ok(grip)
    }
}

impl Drop for Grip {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(self.owner, Some(cursor_proc), SUBCLASS);
            if GetCursor() == self.cursor {
                SetCursor(self.previous);
            }
            let _ = DestroyCursor(self.cursor);
        }
    }
}

fn holding(owner: HWND) -> bool {
    unsafe { GetCapture() == owner && GetAsyncKeyState(i32::from(VK_LBUTTON.0)) < 0 }
}

pub(super) unsafe extern "system" fn cursor_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    cursor: usize,
) -> LRESULT {
    let captured = holding(hwnd);
    if message == WM_SETCURSOR && captured {
        unsafe {
            SetCursor(Some(HCURSOR(cursor as *mut _)));
        }
        return LRESULT(1);
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    // GPUI can update its cursor during mouse dispatch; keep the grip until capture ends.
    let mut active = 0;
    if holding(hwnd)
        && unsafe {
            GetWindowSubclass(hwnd, Some(cursor_proc), SUBCLASS, Some(&mut active)).as_bool()
        }
    {
        unsafe {
            SetCursor(Some(HCURSOR(active as *mut _)));
        }
    }
    result
}

fn masks() -> ([u8; 128], [u8; 128]) {
    let mut and = [255; 128];
    let mut xor = [0; 128];
    for (row, line) in HAND.iter().enumerate() {
        for (column, pixel) in line.bytes().enumerate() {
            if pixel == b' ' {
                continue;
            }
            let x = column + 3;
            let y = row + 4;
            let byte = y * 4 + x / 8;
            let bit = 1 << (7 - x % 8);
            and[byte] &= !bit;
            if pixel == b'.' {
                xor[byte] |= bit;
            }
        }
    }
    (and, xor)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gripping_cursor_has_transparent_padding_and_opaque_black_and_white_hand() {
        let (and, xor) = masks();
        assert_eq!(&and[..16], &[255; 16]);
        assert_eq!(&xor[..16], &[0; 16]);
        assert!(and.iter().any(|byte| *byte != 255));
        assert!(xor.iter().any(|byte| *byte != 0));
        assert!(and.iter().zip(xor).all(|(and, xor)| and & xor == 0));
    }
}
