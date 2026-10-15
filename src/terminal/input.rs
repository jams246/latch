use alacritty_terminal::term::TermMode;
use gpui::Keystroke;

pub(super) fn encode(key: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let modifiers = key.modifiers;
    let modifier = 1
        + usize::from(modifiers.shift)
        + 2 * usize::from(modifiers.alt)
        + 4 * usize::from(modifiers.control);
    let modified = modifier > 1;
    let arrow = match key.key.as_str() {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        _ => None,
    };
    if let Some(code) = arrow {
        return Some(
            if modified {
                format!("\x1b[1;{modifier}{code}")
            } else if mode.contains(TermMode::APP_CURSOR) {
                format!("\x1bO{code}")
            } else {
                format!("\x1b[{code}")
            }
            .into_bytes(),
        );
    }
    let number = match key.key.as_str() {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(number) = number {
        return Some(
            if modified {
                format!("\x1b[{number};{modifier}~")
            } else {
                format!("\x1b[{number}~")
            }
            .into_bytes(),
        );
    }
    let mut bytes = match key.key.as_str() {
        "enter" => vec![b'\r'],
        "escape" => vec![0x1b],
        "backspace" => vec![0x7f],
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" => vec![b'\t'],
        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        "space" if modifiers.control => vec![0],
        _ if modifiers.control && key.key.len() == 1 => {
            let byte = key.key.as_bytes()[0].to_ascii_uppercase();
            if !(b'@'..=b'_').contains(&byte) {
                return None;
            }
            vec![byte & 0x1f]
        }
        _ if modifiers.alt && !modifiers.platform => key.key_char.as_ref()?.as_bytes().to_vec(),
        _ => return None,
    };
    if modifiers.alt && key.key != "escape" {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

pub(super) fn paste(text: &str, mode: TermMode) -> Vec<u8> {
    let text = text.replace('\x1b', "").replace("\r\n", "\n");
    if mode.contains(TermMode::BRACKETED_PASTE) {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_should_follow_application_cursor_mode() {
        let key = Keystroke::parse("up").unwrap();
        assert_eq!(encode(&key, TermMode::APP_CURSOR), Some(b"\x1bOA".to_vec()));
    }

    #[test]
    fn control_c_should_send_interrupt() {
        let key = Keystroke::parse("ctrl-c").unwrap();
        assert_eq!(encode(&key, TermMode::empty()), Some(vec![3]));
    }

    #[test]
    fn modified_arrow_should_encode_modifiers() {
        let key = Keystroke::parse("ctrl-right").unwrap();
        assert_eq!(encode(&key, TermMode::empty()), Some(b"\x1b[1;5C".to_vec()));
    }

    #[test]
    fn bracketed_paste_should_remove_escape_injection() {
        assert_eq!(
            paste("hi\x1b[201~\r\n", TermMode::BRACKETED_PASTE),
            b"\x1b[200~hi[201~\n\x1b[201~"
        );
    }
}
