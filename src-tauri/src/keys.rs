//! Key / mouse-button names used by the UI -> virtual-key codes. Pure logic, no OS calls.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mouse {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Vk(u16),
    Mouse(Mouse),
}

pub fn parse(name: &str) -> Option<Key> {
    let n = name.trim();
    match n {
        "MouseLeft" => return Some(Key::Mouse(Mouse::Left)),
        "MouseRight" => return Some(Key::Mouse(Mouse::Right)),
        "MouseMiddle" => return Some(Key::Mouse(Mouse::Middle)),
        "MouseX1" => return Some(Key::Mouse(Mouse::X1)),
        "MouseX2" => return Some(Key::Mouse(Mouse::X2)),
        _ => {}
    }
    let vk: u16 = match n {
        "Space" => 0x20,
        "Enter" => 0x0D,
        "Tab" => 0x09,
        "Shift" => 0x10,
        "Ctrl" => 0x11,
        "Alt" => 0x12,
        "Escape" => 0x1B,
        "Backspace" => 0x08,
        "CapsLock" => 0x14,
        "Up" => 0x26,
        "Down" => 0x28,
        "Left" => 0x25,
        "Right" => 0x27,
        "Insert" => 0x2D,
        "Delete" => 0x2E,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "Semicolon" => 0xBA,
        "Equal" => 0xBB,
        "Comma" => 0xBC,
        "Minus" => 0xBD,
        "Period" => 0xBE,
        "Slash" => 0xBF,
        "Backquote" => 0xC0,
        "BracketLeft" => 0xDB,
        "Backslash" => 0xDC,
        "BracketRight" => 0xDD,
        "Quote" => 0xDE,
        _ => {
            let b = n.as_bytes();
            if b.len() == 1 && b[0].is_ascii_uppercase() {
                b[0] as u16 // 'A'..='Z' == 0x41..=0x5A
            } else if b.len() == 1 && b[0].is_ascii_digit() {
                b[0] as u16 // '0'..='9' == 0x30..=0x39
            } else if let Some(d) = n.strip_prefix("Num") {
                match d.parse::<u16>() {
                    Ok(v) if v <= 9 => 0x60 + v,
                    _ => return None,
                }
            } else if let Some(d) = n.strip_prefix('F') {
                match d.parse::<u16>() {
                    Ok(v) if (1..=24).contains(&v) => 0x70 + v - 1,
                    _ => return None,
                }
            } else {
                return None;
            }
        }
    };
    Some(Key::Vk(vk))
}

/// Virtual-key code to poll with GetAsyncKeyState.
pub fn poll_vk(k: Key) -> u16 {
    match k {
        Key::Vk(v) => v,
        Key::Mouse(Mouse::Left) => 0x01,
        Key::Mouse(Mouse::Right) => 0x02,
        Key::Mouse(Mouse::Middle) => 0x04,
        Key::Mouse(Mouse::X1) => 0x05,
        Key::Mouse(Mouse::X2) => 0x06,
    }
}

/// Keys that need KEYEVENTF_EXTENDEDKEY.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_extended(vk: u16) -> bool {
    matches!(vk, 0x21..=0x28 | 0x2D | 0x2E)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(parse("A"), Some(Key::Vk(0x41)));
        assert_eq!(parse("Z"), Some(Key::Vk(0x5A)));
        assert_eq!(parse("0"), Some(Key::Vk(0x30)));
        assert_eq!(parse("F1"), Some(Key::Vk(0x70)));
        assert_eq!(parse("F6"), Some(Key::Vk(0x75)));
        assert_eq!(parse("F24"), Some(Key::Vk(0x87)));
        assert_eq!(parse("F25"), None);
        assert_eq!(parse("Num5"), Some(Key::Vk(0x65)));
        assert_eq!(parse("MouseX1"), Some(Key::Mouse(Mouse::X1)));
        assert_eq!(parse("nonsense"), None);
        assert_eq!(parse("a"), None);
        assert_eq!(poll_vk(Key::Mouse(Mouse::Right)), 2);
        assert!(is_extended(0x26) && !is_extended(0x41));
    }
}
