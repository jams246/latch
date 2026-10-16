use std::{path::Path, sync::Arc};
use windows::{
    Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, HBITMAP, HDC, HGDIOBJ, SelectObject,
        },
        Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL},
        UI::{
            Shell::{
                SHFILEINFOW, SHGFI_ICON, SHGFI_LINKOVERLAY, SHGFI_SMALLICON,
                SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
            },
            WindowsAndMessaging::{DI_MASK, DI_NORMAL, DestroyIcon, DrawIconEx, HICON},
        },
    },
    core::PCWSTR,
};

const SIDE: usize = 16;
const BYTE_LENGTH: usize = SIDE * SIDE * 4;

struct Icon(HICON);

impl Drop for Icon {
    fn drop(&mut self) {
        let _ = unsafe { DestroyIcon(self.0) };
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    pixels: *mut u8,
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

impl Surface {
    fn new() -> Option<Self> {
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.is_invalid() {
            return None;
        }
        let information = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIDE as i32,
                biHeight: -(SIDE as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(Some(dc), &information, DIB_RGB_COLORS, &mut pixels, None, 0)
        };
        let Ok(bitmap) = bitmap else {
            let _ = unsafe { DeleteDC(dc) };
            return None;
        };
        let previous = unsafe { SelectObject(dc, bitmap.into()) };
        if previous.is_invalid() {
            unsafe {
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(dc);
            }
            return None;
        }
        Some(Self {
            dc,
            bitmap,
            previous,
            pixels: pixels.cast(),
        })
    }

    fn draw(
        &mut self,
        icon: HICON,
        flags: windows::Win32::UI::WindowsAndMessaging::DI_FLAGS,
    ) -> Option<Vec<u8>> {
        // The DIB owns exactly SIDE * SIDE writable BGRA pixels until this surface drops.
        unsafe {
            std::ptr::write_bytes(self.pixels, 0, BYTE_LENGTH);
            DrawIconEx(
                self.dc,
                0,
                0,
                icon,
                SIDE as i32,
                SIDE as i32,
                0,
                None,
                flags,
            )
            .ok()?;
            if !GdiFlush().as_bool() {
                return None;
            }
            Some(std::slice::from_raw_parts(self.pixels, BYTE_LENGTH).to_vec())
        }
    }
}

pub(in crate::ui::files::local) fn icon(
    path: &Path,
    is_dir: bool,
    is_link: bool,
) -> Option<Arc<gpui::RenderImage>> {
    // Concurrent shell extraction can report success with a null icon handle.
    static SHELL_ICONS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SHELL_ICONS.lock().ok()?;
    let path = super::wide(path).ok()?;
    let mut information = SHFILEINFOW::default();
    let attributes = if is_dir {
        FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    let mut flags = SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES;
    if is_link {
        flags |= SHGFI_LINKOVERLAY;
    }
    // Registered file-type icons avoid slow or unavailable per-file shell handlers.
    let result = unsafe {
        SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            attributes,
            Some(&mut information),
            size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };
    if result == 0 || information.hIcon.is_invalid() {
        return None;
    }
    let icon = Icon(information.hIcon);
    let mut surface = Surface::new()?;
    let mut pixels = surface.draw(icon.0, DI_NORMAL)?;
    if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
        let mask = surface.draw(icon.0, DI_MASK)?;
        for (pixel, mask) in pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(mask.as_chunks::<4>().0)
        {
            pixel[3] = if mask[0] == 0 { 255 } else { 0 };
        }
    } else {
        // DrawIconEx supplies premultiplied pixels; GPUI expects straight BGRA channels.
        for pixel in pixels.as_chunks_mut::<4>().0 {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = (u32::from(*channel) * 255)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
    }
    let buffer = image::ImageBuffer::from_raw(SIDE as u32, SIDE as u32, pixels)?;
    Some(Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        buffer,
    )])))
}
