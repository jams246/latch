use std::{
    fs, io,
    io::{Read, Write},
    path::Path,
};

use zeroize::Zeroizing;

use super::invalid;

pub(super) fn create(path: &Path) -> io::Result<()> {
    let key = generate()?;
    let protected = protect(&key)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&protected)?;
    file.sync_all()
}

pub(super) fn read(path: &Path) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err(invalid());
    }
    let key = unprotect(&bytes)?;
    if key.len() != 32 {
        return Err(invalid());
    }
    Ok(key)
}

#[cfg(windows)]
fn generate() -> io::Result<Zeroizing<Vec<u8>>> {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut bytes = Zeroizing::new(vec![0; 32]);
    // SAFETY: Windows writes only into the provided live output slice.
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|_| invalid())?;
    Ok(bytes)
}

#[cfg(windows)]
fn protect(bytes: &[u8]) -> io::Result<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };
    let input = blob(bytes)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: Input lives for this call; DPAPI owns output until copy_output frees it.
    unsafe {
        CryptProtectData(
            &input,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|_| invalid())?;
    Ok(copy_output(output).to_vec())
}

#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };
    let input = blob(bytes)?;
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: Input lives for this call; DPAPI owns output until copy_output frees it.
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    }
    .map_err(|_| invalid())?;
    Ok(copy_output(output))
}

#[cfg(windows)]
fn blob(bytes: &[u8]) -> io::Result<windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB> {
    Ok(windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB {
        cbData: bytes.len().try_into().map_err(|_| invalid())?,
        pbData: bytes.as_ptr().cast_mut(),
    })
}

#[cfg(windows)]
fn copy_output(
    output: windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
) -> Zeroizing<Vec<u8>> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use zeroize::Zeroize;
    if output.pbData.is_null() {
        return Zeroizing::new(Vec::new());
    }
    // SAFETY: DPAPI returned this allocation and its length; LocalFree releases it once.
    unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let copy = Zeroizing::new(bytes.to_vec());
        bytes.zeroize();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        copy
    }
}

#[cfg(not(windows))]
fn generate() -> io::Result<Zeroizing<Vec<u8>>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows DPAPI is required",
    ))
}

#[cfg(not(windows))]
fn protect(_: &[u8]) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows DPAPI is required",
    ))
}

#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows DPAPI is required",
    ))
}
