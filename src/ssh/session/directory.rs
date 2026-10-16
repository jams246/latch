use alacritty_terminal::vte::{Parser, Perform};

const MAX_OSC_BYTES: usize = 4096;

/// Remembers only a validated directory reported by this tab's remote shell.
#[derive(Default)]
pub(super) struct Directory {
    parser: Parser,
    observed: Observed,
    escape: bool,
    osc: bool,
}

impl Directory {
    pub(super) fn feed(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if *byte != b'\\' {
                self.observed.pending = None;
            }
            if self.escape && *byte == b']' {
                self.osc = true;
                self.observed.invalid = false;
                self.observed.bytes = 0;
                self.observed.separators = 0;
            } else if self.osc {
                // VTE omits raw C0 bytes and excess parameters from OSC callbacks.
                self.observed.bytes = (self.observed.bytes + 1).min(MAX_OSC_BYTES + 1);
                self.observed.invalid |= self.observed.bytes > MAX_OSC_BYTES
                    || (byte.is_ascii_control() && !matches!(byte, b'\x07' | b'\x1b'));
                if *byte == b';' {
                    self.observed.separators = (self.observed.separators + 1).min(16);
                    self.observed.invalid |= self.observed.separators == 16;
                }
                if matches!(byte, b'\x07' | b'\x1b' | b'\x18' | b'\x1a') {
                    self.osc = false;
                }
            }
            self.escape = match byte {
                b'\x1b' => true,
                b'\x18' | b'\x1a' | 0x20..=0x7e => false,
                _ => self.escape,
            };
            self.parser
                .advance(&mut self.observed, std::slice::from_ref(byte));
        }
    }

    pub(super) fn path(&self) -> Option<&str> {
        self.observed.path.as_deref()
    }

    pub(super) fn reset_parser(&mut self) {
        let path = self.observed.path.take();
        *self = Self::default();
        self.observed.path = path;
    }
}

#[derive(Default)]
struct Observed {
    path: Option<String>,
    pending: Option<String>,
    invalid: bool,
    bytes: usize,
    separators: usize,
}

impl Perform for Observed {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        self.pending = None;
        if self.invalid {
            return;
        }
        if let Some(path) = directory_path(params) {
            if bell_terminated {
                self.path = Some(path);
            } else {
                // VTE dispatches on ESC; wait for the remaining ST backslash.
                self.pending = Some(path);
            }
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        if let Some(path) = self.pending.take()
            && byte == b'\\'
            && intermediates.is_empty()
            && !ignore
        {
            self.path = Some(path);
        }
    }
}

fn directory_path(params: &[&[u8]]) -> Option<String> {
    if params.first().copied()? != b"7" || params.len() < 2 {
        return None;
    }
    let size = params[1..].iter().map(|param| param.len()).sum::<usize>() + params.len() - 2;
    if size > MAX_OSC_BYTES {
        return None;
    }
    let mut uri = Vec::with_capacity(size);
    for (index, param) in params[1..].iter().enumerate() {
        if index != 0 {
            uri.push(b';');
        }
        uri.extend_from_slice(param);
    }
    if uri.iter().any(|byte| matches!(byte, b'?' | b'#')) {
        return None;
    }
    let location = uri.strip_prefix(b"file://")?;
    let slash = location.iter().position(|byte| *byte == b'/')?;
    decode(&location[..slash])?;
    let path = decode(&location[slash..])?;
    path.starts_with('/').then_some(path)
}

fn decode(value: &[u8]) -> Option<String> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut bytes = value.iter().copied();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            hex(bytes.next()?)? * 16 + hex(bytes.next()?)?
        } else {
            byte
        });
    }
    let value = String::from_utf8(decoded).ok()?;
    (!value.chars().any(char::is_control)).then_some(value)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
