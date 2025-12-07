use crate::registry_comparator::{DiffResult, KeyPropertyChange, RegistryChange};
use crate::registry_value::{RegistryValue, RegistryValueData, REG_BINARY};

pub struct TextDiffExporter;

impl TextDiffExporter {
    pub fn export(&self, diff: &DiffResult, from_file: Option<&str>, to_file: Option<&str>) -> String {
        let mut out = String::new();
        out.push_str("# Registry Patch File\n");
        out.push_str("# Generated: ");
        out.push_str(&chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        out.push('\n');
        if let (Some(f1), Some(f2)) = (from_file, to_file) {
            out.push_str("# FROM: ");
            out.push_str(f1);
            out.push('\n');
            out.push_str("# TO: ");
            out.push_str(f2);
            out.push('\n');
        }
        out.push('\n');
        if !diff.has_changes() {
            out.push_str("# No changes\n");
            return out;
        }

        let mut grouped: std::collections::BTreeMap<String, Vec<RegistryChange>> = std::collections::BTreeMap::new();
        for change in &diff.changes {
            let key = match change {
                RegistryChange::KeyAdded(p) => parent_path(p),
                RegistryChange::KeyDeleted(p) => parent_path(p),
                RegistryChange::KeyModified(p, _) => p.clone(),
                RegistryChange::ValueAdded(k, _, _) => k.clone(),
                RegistryChange::ValueDeleted(k, _, _) => k.clone(),
                RegistryChange::ValueModified(k, _, _, _) => k.clone(),
            };
            grouped.entry(key).or_default().push(change.clone());
        }

        for (path, changes) in grouped {
            let header = if path.is_empty() {
                "[ROOT]".to_string()
            } else {
                format!("[{}]", path)
            };
            out.push_str(&header);
            out.push('\n');
            for c in changes {
                match c {
                    RegistryChange::KeyAdded(p) => {
                        let name = leaf_name(&p);
                        out.push_str("+key:");
                        out.push_str(&name);
                        out.push('\n');
                    }
                    RegistryChange::KeyDeleted(p) => {
                        let name = leaf_name(&p);
                        out.push_str("-key:");
                        out.push_str(&name);
                        out.push('\n');
                    }
                    RegistryChange::KeyModified(_, props) => {
                        for prop in props {
                            match prop {
                                KeyPropertyChange::ClassNameChange(old, newv) => {
                                    out.push_str("~className:");
                                    out.push_str(&format_property(&old));
                                    out.push_str("->");
                                    out.push_str(&format_property(&newv));
                                    out.push('\n');
                                }
                                KeyPropertyChange::SymlinkChange(old, newv) => {
                                    out.push_str(&format!("~isSymlink:{}->{}\n", old, newv));
                                }
                                KeyPropertyChange::VolatileChange(old, newv) => {
                                    out.push_str(&format!("~isVolatile:{}->{}\n", old, newv));
                                }
                            }
                        }
                    }
                    RegistryChange::ValueAdded(_, name, value) => {
                        out.push('+');
                        out.push_str(&format_value(&name, &value));
                        out.push('\n');
                    }
                    RegistryChange::ValueDeleted(_, name, value) => {
                        out.push('-');
                        out.push_str(&format_value(&name, &value));
                        out.push('\n');
                    }
                    RegistryChange::ValueModified(_, name, old, newv) => {
                        out.push('~');
                        out.push('"');
                        out.push_str(&escape_string(&name));
                        out.push_str("\"=");
                        out.push_str(&format_value_data(&old));
                        out.push_str("->");
                        out.push_str(&format_value_data(&newv));
                        out.push('\n');
                    }
                }
            }
            out.push('\n');
        }

        out
    }
}

pub struct TextDiffParser;

impl TextDiffParser {
    pub fn parse(&self, text: &str) -> Result<DiffResult, String> {
        let mut path = String::new();
        let mut changes = Vec::new();
        let mut key_props: std::collections::BTreeMap<String, Vec<KeyPropertyChange>> = std::collections::BTreeMap::new();

        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                path = trimmed[1..trimmed.len() - 1].to_string();
                if path == "ROOT" {
                    path.clear();
                }
                continue;
            }
            if trimmed.starts_with("+key:") {
                let name = trimmed["+key:".len()..].to_string();
                let full = join_path(&path, &name);
                changes.push(RegistryChange::KeyAdded(full));
                continue;
            }
            if trimmed.starts_with("-key:") {
                let name = trimmed["-key:".len()..].to_string();
                let full = join_path(&path, &name);
                changes.push(RegistryChange::KeyDeleted(full));
                continue;
            }
            if trimmed.starts_with("~className:") {
                let rest = &trimmed["~className:".len()..];
                let (old, newv) = split_arrow(rest)?;
                key_props.entry(path.clone()).or_default().push(KeyPropertyChange::ClassNameChange(parse_property_value(old), parse_property_value(newv)));
                continue;
            }
            if trimmed.starts_with("~isSymlink:") {
                let (old, newv) = split_arrow(&trimmed["~isSymlink:".len()..])?;
                let old_b = old.trim().parse::<bool>().map_err(|_| format!("line {}", idx + 1))?;
                let new_b = newv.trim().parse::<bool>().map_err(|_| format!("line {}", idx + 1))?;
                key_props.entry(path.clone()).or_default().push(KeyPropertyChange::SymlinkChange(old_b, new_b));
                continue;
            }
            if trimmed.starts_with("~isVolatile:") {
                let (old, newv) = split_arrow(&trimmed["~isVolatile:".len()..])?;
                let old_b = old.trim().parse::<bool>().map_err(|_| format!("line {}", idx + 1))?;
                let new_b = newv.trim().parse::<bool>().map_err(|_| format!("line {}", idx + 1))?;
                key_props.entry(path.clone()).or_default().push(KeyPropertyChange::VolatileChange(old_b, new_b));
                continue;
            }
            if trimmed.starts_with("+\"") || trimmed.starts_with("-\"") {
                let add = trimmed.starts_with('+');
                let val_part = &trimmed[1..];
                let (name, value) = parse_value(val_part)?;
                let full_change = if add {
                    RegistryChange::ValueAdded(path.clone(), name, value)
                } else {
                    RegistryChange::ValueDeleted(path.clone(), name, value)
                };
                changes.push(full_change);
                continue;
            }
            if trimmed.starts_with("~\"") {
                let val_part = &trimmed[1..];
                let (name, old_value, new_value) = parse_value_modification(val_part)?;
                changes.push(RegistryChange::ValueModified(path.clone(), name, old_value, new_value));
                continue;
            }
        }

        for (path, props) in key_props {
            changes.push(RegistryChange::KeyModified(path, props));
        }

        Ok(DiffResult { changes })
    }
}

fn parent_path(path: &str) -> String {
    path.rsplit_once('\\').map(|(p, _)| p.to_string()).unwrap_or_else(|| "".into())
}

fn leaf_name(path: &str) -> String {
    path.rsplit_once('\\').map(|(_, n)| n.to_string()).unwrap_or_else(|| path.to_string())
}

fn join_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{}\\{}", base, name)
    }
}

fn format_property(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("\"{}\"", escape_string(s)),
        None => "null".into(),
    }
}

fn format_value(name: &str, value: &RegistryValue) -> String {
    let mut s = String::new();
    s.push('"');
    s.push_str(&escape_string(name));
    s.push_str("\"=");
    s.push_str(&format_value_data(value));
    s
}

fn format_value_data(value: &RegistryValue) -> String {
    match &value.data {
        RegistryValueData::String(v) => format!("string:\"{}\"", escape_string(v)),
        RegistryValueData::ExpandString(v) => format!("expand_string:\"{}\"", escape_string(v)),
        RegistryValueData::MultiString(vs) => {
            let joined = vs.iter().map(|s| format!("\"{}\"", escape_string(s))).collect::<Vec<_>>().join(",");
            format!("multi_string:[{}]", joined)
        }
        RegistryValueData::Dword(v) => format!("dword:{:08x}", v),
        RegistryValueData::Qword(v) => format!("qword:{:016x}", v),
        RegistryValueData::Binary(bytes, ty) => {
            let prefix = if *ty == REG_BINARY { "hex:".to_string() } else { format!("hex({:x}):", ty) };
            let body = bytes.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(",");
            format!("{}{}", prefix, body)
        }
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn split_arrow(s: &str) -> Result<(&str, &str), String> {
    let pos = s.find("->").ok_or_else(|| "missing ->".to_string())?;
    Ok((&s[..pos], &s[pos + 2..]))
}

fn parse_property_value(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed == "null" {
        None
    } else {
        Some(trimmed.trim_matches('"').to_string())
    }
}

fn parse_value(line: &str) -> Result<(String, RegistryValue), String> {
    let eq = line.find('=').ok_or("invalid value line")?;
    let raw_name = &line[..eq];
    let name = raw_name.trim_matches('"').to_string();
    let value_part = &line[eq + 1..];
    let value = parse_value_data_part(value_part)?;
    Ok((unescape(&name), value))
}

fn parse_value_modification(line: &str) -> Result<(String, RegistryValue, RegistryValue), String> {
    let eq = line.find('=').ok_or("invalid modification")?;
    let raw_name = &line[..eq];
    let name = unescape(raw_name.trim_matches('"'));
    let rest = &line[eq + 1..];
    let (old_part, new_part) = split_arrow(rest)?;
    let old_val = parse_value_data_part(old_part)?;
    let new_val = parse_value_data_part(new_part)?;
    Ok((name, old_val, new_val))
}

fn parse_value_data_part(data: &str) -> Result<RegistryValue, String> {
    let trimmed = data.trim();
    if trimmed.starts_with("string:") {
        let s = trimmed["string:".len()..].trim().trim_matches('"').to_string();
        return Ok(RegistryValue::new("", RegistryValueData::String(unescape(&s))));
    }
    if trimmed.starts_with("expand_string:") {
        let s = trimmed["expand_string:".len()..].trim().trim_matches('"').to_string();
        return Ok(RegistryValue::new("", RegistryValueData::ExpandString(unescape(&s))));
    }
    if trimmed.starts_with("multi_string:") {
        let content = trimmed["multi_string:".len()..].trim();
        let inner = content.trim_matches(['[', ']'].as_ref());
        let mut values = Vec::new();
        if !inner.is_empty() {
            for part in inner.split(',') {
                let v = part.trim().trim_matches('"');
                values.push(unescape(v));
            }
        }
        return Ok(RegistryValue::new("", RegistryValueData::MultiString(values)));
    }
    if trimmed.starts_with("dword:") {
        let v = u32::from_str_radix(trimmed["dword:".len()..].trim(), 16).map_err(|e| e.to_string())?;
        return Ok(RegistryValue::new("", RegistryValueData::Dword(v)));
    }
    if trimmed.starts_with("qword:") {
        let v = u64::from_str_radix(trimmed["qword:".len()..].trim(), 16).map_err(|e| e.to_string())?;
        return Ok(RegistryValue::new("", RegistryValueData::Qword(v)));
    }
    if trimmed.starts_with("hex(") {
        let end = trimmed.find("):").ok_or("bad hex")?;
        let ty = u32::from_str_radix(&trimmed[4..end], 16).map_err(|e| e.to_string())?;
        let bytes = parse_hex_bytes(&trimmed[end + 2..])?;
        return Ok(RegistryValue::new("", RegistryValueData::Binary(bytes, ty)));
    }
    if trimmed.starts_with("hex:") {
        let bytes = parse_hex_bytes(&trimmed["hex:".len()..])?;
        return Ok(RegistryValue::new("", RegistryValueData::Binary(bytes, REG_BINARY)));
    }
    Err("unknown value format".into())
}

fn parse_hex_bytes(data: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    if data.trim().is_empty() {
        return Ok(bytes);
    }
    for part in data.split(',') {
        let byte = u8::from_str_radix(part.trim(), 16).map_err(|e| e.to_string())?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                match n {
                    '\\' => out.push('\\'),
                    '"' => out.push('"'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    '0' => out.push('\u{0}'),
                    _ => out.push(n),
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

