mod icons;

pub(super) use icons::icon;

use std::{
    ffi::OsString,
    io,
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        Storage::FileSystem::GetLogicalDrives,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, STGM_READ,
        },
        UI::Shell::{IShellLinkW, SLGP_UNCPRIORITY, ShellLink},
    },
    core::{Interface, PCWSTR},
};

pub(super) struct Apartment(bool);

impl Apartment {
    pub(super) fn new() -> io::Result<Self> {
        // Blocking workers can already belong to an apartment initialized by GPUI.
        let status =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if status == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        status.ok().map_err(io::Error::other)?;
        Ok(Self(true))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub(super) fn drives() -> Vec<PathBuf> {
    let mask = unsafe { GetLogicalDrives() };
    (0..26)
        .filter(|index| mask & (1 << index) != 0)
        .map(|index| PathBuf::from(format!("{}:\\", char::from(b'A' + index))))
        .collect()
}

pub(super) fn resolve_shortcut(path: &Path) -> io::Result<PathBuf> {
    let _apartment = Apartment::new()?;
    let path = wide(path)?;
    // Read the persisted target without ShellExecute, resolution dialogs, or target execution.
    unsafe {
        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(io::Error::other)?;
        let file: IPersistFile = link.cast().map_err(io::Error::other)?;
        file.Load(PCWSTR(path.as_ptr()), STGM_READ)
            .map_err(io::Error::other)?;
        let mut target = vec![0; 32_768];
        link.GetPath(&mut target, std::ptr::null_mut(), SLGP_UNCPRIORITY.0 as u32)
            .map_err(io::Error::other)?;
        let length = target
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(target.len());
        if length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "The shortcut does not target a filesystem path",
            ));
        }
        Ok(PathBuf::from(OsString::from_wide(&target[..length])))
    }
}

pub(super) fn display_path(path: &Path) -> PathBuf {
    let path: Vec<u16> = path.as_os_str().encode_wide().collect();
    let extended_unc: Vec<u16> = r"\\?\UNC\".encode_utf16().collect();
    let extended: Vec<u16> = r"\\?\".encode_utf16().collect();
    if let Some(tail) = path.strip_prefix(extended_unc.as_slice()) {
        let mut result: Vec<u16> = r"\\".encode_utf16().collect();
        result.extend_from_slice(tail);
        PathBuf::from(OsString::from_wide(&result))
    } else if let Some(tail) = path.strip_prefix(extended.as_slice()) {
        PathBuf::from(OsString::from_wide(tail))
    } else {
        PathBuf::from(OsString::from_wide(&path))
    }
}

pub(super) fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut text: Vec<u16> = path.as_os_str().encode_wide().collect();
    if text.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path contains a null character",
        ));
    }
    text.push(0);
    Ok(text)
}
