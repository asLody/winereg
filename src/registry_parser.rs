use std::fs;
use std::path::Path;

use crate::architecture::Architecture;
use crate::registry_key::{KeyNode, RegistryKey};
use crate::registry_utils::{timestamp_to_filetime};
use crate::registry_value::{RegistryValue, RegistryValueData, REG_BINARY, REG_QWORD};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid header")]
    InvalidHeader,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at line {line}: {msg}")]
    Line { line: usize, msg: String },
}

#[derive(Debug)]
pub struct LoadResult {
    pub root_key: KeyNode,
    pub relative_base: String,
    pub architecture: Architecture,
}

pub struct RegistryParser;

impl RegistryParser {
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<LoadResult, ParseError> {
        let text = fs::read_to_string(path)?;
        self.load_from_text(&text)
    }

    pub fn load_from_text(&self, text: &str) -> Result<LoadResult, ParseError> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return Err(ParseError::InvalidHeader);
        }
        let mut line_idx = 0usize;

        // header
        if lines[0].trim() != "WINE REGISTRY Version 2" {
            return Err(ParseError::InvalidHeader);
        }
        line_idx += 1;

        let root = RegistryKey::create_root();
        let mut relative_base = String::new();
        let mut architecture = Architecture::Unknown;
        let mut current_key: Option<KeyNode> = None;

        while line_idx < lines.len() {
            let raw = lines[line_idx];
            let trimmed = raw.trim();
            line_idx += 1;
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with(";; All keys relative to ") {
                relative_base = trimmed[";; All keys relative to ".len()..].to_string();
                continue;
            }
            if trimmed.starts_with(';') {
                continue;
            }
            if trimmed.starts_with("#arch=") {
                if let Some(a) = Architecture::from_tag(&trimmed["#arch=".len()..]) {
                    architecture = a;
                }
                continue;
            }
            if trimmed.starts_with('[') {
                let (path, timestamp) = parse_key_header(trimmed).map_err(|msg| ParseError::Line { line: line_idx, msg })?;
                let key_path = unescape_key_path(&path);
                let key_node = RegistryKey::create_key_recursive(&root, &key_path);
                {
                    let mut guard = key_node.borrow_mut();
                    guard.modification_time = timestamp_to_filetime(timestamp);
                }
                current_key = Some(key_node);
                continue;
            }
            if trimmed.starts_with("#time=") {
                if let Some(ref key) = current_key {
                    if let Ok(val) = u64::from_str_radix(trimmed["#time=".len()..].trim(), 16) {
                        key.borrow_mut().modification_time = val;
                    }
                }
                continue;
            }
            if trimmed.starts_with("#class=") {
                if let Some(ref key) = current_key {
                    let cls = trimmed["#class=".len()..].trim();
                    let unquoted = cls.trim_matches('"').to_string();
                    key.borrow_mut().class_name = Some(unescape_string(&unquoted));
                }
                continue;
            }
            if trimmed.starts_with("#link") {
                if let Some(ref key) = current_key {
                    key.borrow_mut().is_symlink = true;
                }
                continue;
            }

            // value line
            if trimmed.starts_with('@') || trimmed.starts_with('"') {
                let (value, consumed) = parse_value_line(trimmed, &lines[(line_idx)..]).map_err(|msg| ParseError::Line { line: line_idx, msg })?;
                if let Some(ref key) = current_key {
                    key.borrow_mut().set_value_for_loading(value.name.clone(), value);
                } else {
                    return Err(ParseError::Line { line: line_idx, msg: "value without key".into() });
                }
                line_idx += consumed;
                continue;
            }

            // unknown line - skip
        }

        Ok(LoadResult {
            root_key: root,
            relative_base,
            architecture,
        })
    }
}

fn parse_key_header(line: &str) -> Result<(String, u64), String> {
    if !line.starts_with('[') || !line.contains(']') {
        return Err(format!("malformed key header: {}", line));
    }
    let end = line.find(']').unwrap();
    let key_path = line[1..end].trim().to_string();
    let rest = line[end + 1..].trim();
    let timestamp = if rest.is_empty() {
        0
    } else {
        rest.parse::<u64>().unwrap_or(0)
    };
    Ok((key_path, timestamp))
}

fn parse_value_line(first_line: &str, rest: &[&str]) -> Result<(RegistryValue, usize), String> {
    let mut buffer = String::new();
    buffer.push_str(first_line.trim_end());
    let mut consumed = 0usize;

    // consume continuation lines for hex data
    while buffer.trim_end().ends_with('\\') {
        buffer = buffer.trim_end_matches('\\').trim_end().to_string();
        if consumed >= rest.len() {
            break;
        }
        buffer.push_str(rest[consumed].trim());
        consumed += 1;
    }

    let name;
    let cursor: usize;
    if buffer.starts_with("@=") {
        name = String::new();
        cursor = 1; // position at '='
    } else if buffer.starts_with('"') {
        let mut i = 1;
        let bytes = buffer.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                break;
            }
            i += 1;
        }
        if i >= bytes.len() {
            return Err("unterminated value name".into());
        }
        let raw_name = &buffer[1..i];
        name = unescape_string(raw_name);
        cursor = i;
    } else {
        return Err("invalid value line".into());
    }

    let mut after_name = buffer[cursor + 1..].trim_start(); // skip '='
    if after_name.starts_with('=') {
        after_name = after_name[1..].trim_start();
    }
    let value = parse_value_data(after_name, name.clone())?;
    Ok((value, consumed))
}

fn parse_value_data(data: &str, name: String) -> Result<RegistryValue, String> {
    if data.starts_with("str(2):") {
        let s = parse_quoted_string(&data["str(2):".len()..])?;
        return Ok(RegistryValue::new(name, RegistryValueData::ExpandString(s)));
    }
    if data.starts_with("str(7):") {
        let s = parse_quoted_string(&data["str(7):".len()..])?;
        let parts: Vec<String> = s.split('\u{0}').filter(|v| !v.is_empty()).map(|v| v.to_string()).collect();
        return Ok(RegistryValue::new(name, RegistryValueData::MultiString(parts)));
    }
    if data.starts_with("dword:") {
        let hex = data["dword:".len()..].trim();
        let val = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
        return Ok(RegistryValue::new(name, RegistryValueData::Dword(val)));
    }
    if data.starts_with("qword:") {
        let hex = data["qword:".len()..].trim();
        let val = u64::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
        return Ok(RegistryValue::new(name, RegistryValueData::Qword(val)));
    }
    if data.starts_with("hex(") {
        let end = data.find("):").ok_or("malformed hex type")?;
        let type_hex = &data[4..end];
        let ty = u32::from_str_radix(type_hex, 16).map_err(|e| e.to_string())?;
        let bytes = parse_hex_bytes(&data[end + 2..])?;
        if ty == REG_QWORD && bytes.len() == 8 {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[..8]);
            let val = u64::from_le_bytes(arr);
            return Ok(RegistryValue::new(name, RegistryValueData::Qword(val)));
        }
        return Ok(RegistryValue::new(name, RegistryValueData::Binary(bytes, ty)));
    }
    if data.starts_with("hex:") {
        let bytes = parse_hex_bytes(&data["hex:".len()..])?;
        return Ok(RegistryValue::new(name, RegistryValueData::Binary(bytes, REG_BINARY)));
    }
    if data.starts_with("hex(b):") {
        let bytes = parse_hex_bytes(&data["hex(b):".len()..])?;
        if bytes.len() == 8 {
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[..8]);
            let val = u64::from_le_bytes(arr);
            return Ok(RegistryValue::new(name, RegistryValueData::Qword(val)));
        }
        return Ok(RegistryValue::new(name, RegistryValueData::Binary(bytes, REG_QWORD)));
    }
    // default string
    let s = parse_quoted_string(data)?;
    Ok(RegistryValue::new(name, RegistryValueData::String(s)))
}

fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for part in s.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let byte = u8::from_str_radix(trimmed, 16).map_err(|e| e.to_string())?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn parse_quoted_string(data: &str) -> Result<String, String> {
    let trimmed = data.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err("expected quoted string".into());
    }
    Ok(unescape_string(&trimmed[1..trimmed.len() - 1]))
}

fn unescape_string(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            match next {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '0' => out.push('\u{0}'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                _ => out.push(next),
            }
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

fn unescape_key_path(s: &str) -> String {
    s.replace("\\\\", "\\")
}

