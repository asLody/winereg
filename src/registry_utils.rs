const TICKS_PER_SEC: u64 = 10_000_000;
const TICKS_1601_TO_1970: u64 = 86_400 * (369 * 365 + 89) * TICKS_PER_SEC;

pub fn timestamp_to_filetime(timestamp: u64) -> u64 {
    timestamp * TICKS_PER_SEC + TICKS_1601_TO_1970
}

pub fn filetime_to_timestamp(filetime: u64) -> u64 {
    (filetime - TICKS_1601_TO_1970) / TICKS_PER_SEC
}

pub fn is_string_type(ty: u32) -> bool {
    matches!(ty, crate::REG_SZ | crate::REG_EXPAND_SZ | crate::REG_MULTI_SZ)
}

pub fn data_type_prefix(ty: u32) -> &'static str {
    match ty {
        crate::REG_SZ => "",
        crate::REG_EXPAND_SZ => "str(2):",
        crate::REG_MULTI_SZ => "str(7):",
        crate::REG_DWORD => "dword:",
        crate::REG_BINARY => "hex:",
        _ => "",
    }
}

pub fn hex_digit_value(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some(c as u8 - b'0'),
        'a'..='f' => Some(c as u8 - b'a' + 10),
        'A'..='F' => Some(c as u8 - b'A' + 10),
        _ => None,
    }
}

pub fn set_current_time_recursive(node: &crate::registry_key::KeyNode) {
    let now = timestamp_to_filetime(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs());
    set_time(node, now);
}

fn set_time(node: &crate::registry_key::KeyNode, time: u64) {
    {
        let mut guard = node.borrow_mut();
        guard.modification_time = time;
    }
    let children: Vec<_> = node.borrow().subkeys().values().cloned().collect();
    for child in children {
        set_time(&child, time);
    }
}

