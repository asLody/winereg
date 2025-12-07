use std::fs;
use std::path::Path;

use crate::architecture::Architecture;
use crate::registry_key::KeyNode;
use crate::registry_utils::filetime_to_timestamp;
use crate::registry_value::{RegistryValueData, REG_BINARY};

pub struct RegistryWriter {
    pub relative_base: String,
    pub architecture: Architecture,
}

impl RegistryWriter {
    pub fn new() -> Self {
        Self {
            relative_base: String::new(),
            architecture: Architecture::Unknown,
        }
    }

    pub fn write_to_string(&self, root: &KeyNode) -> String {
        let mut out = String::new();
        self.write_all(root, &mut out);
        out
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, root: &KeyNode, path: P) -> std::io::Result<()> {
        let content = self.write_to_string(root);
        let mut tmp = path.as_ref().to_path_buf();
        let file_name = tmp.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "registry.reg".into());
        tmp.set_file_name(format!("{}.tmp", file_name));
        fs::write(&tmp, content.as_bytes())?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn write_all(&self, root: &KeyNode, out: &mut String) {
        out.push_str("WINE REGISTRY Version 2\n");
        if !self.relative_base.is_empty() {
            out.push_str(";; All keys relative to ");
            out.push_str(&self.relative_base);
            out.push('\n');
        }
        match self.architecture {
            Architecture::Win32 => out.push_str("\n#arch=win32\n"),
            Architecture::Win64 => out.push_str("\n#arch=win64\n"),
            Architecture::Unknown => {}
        }
        self.write_subkeys(root, root, out);
    }

    fn write_subkeys(&self, node: &KeyNode, base: &KeyNode, out: &mut String) {
        let guard = node.borrow();
        if guard.is_volatile {
            return;
        }

        let values: Vec<_> = guard.values().iter().map(|(_, v)| v.clone()).collect();
        let subkeys: Vec<_> = guard.subkeys().values().cloned().collect();
        let has_meta = guard.class_name.is_some() || guard.is_symlink;

        if !values.is_empty() || subkeys.is_empty() || has_meta {
            out.push('\n');
            out.push('[');
            dump_path(node, base, out);
            out.push_str("] ");
            out.push_str(&filetime_to_timestamp(guard.modification_time).to_string());
            out.push('\n');
            out.push_str("#time=");
            out.push_str(&format!("{:x}", guard.modification_time));
            out.push('\n');
            if let Some(class_name) = &guard.class_name {
                out.push_str("#class=\"");
                out.push_str(&escape_string(class_name));
                out.push_str("\"\n");
            }
            if guard.is_symlink {
                out.push_str("#link\n");
            }
            for value in values {
                dump_value(&value, out);
            }
        }
        drop(guard);

        for sub in subkeys {
            self.write_subkeys(&sub, base, out);
        }
    }
}

fn dump_path(node: &KeyNode, base: &KeyNode, out: &mut String) {
    if Rc::ptr_eq(node, base) {
        return;
    }
    let mut parts = Vec::new();
    let mut current = Some(node.clone());
    while let Some(n) = current {
        let guard = n.borrow();
        if guard.name.is_empty() {
            current = guard.parent();
            continue;
        }
        parts.push(guard.name.clone());
        current = guard.parent();
    }
    parts.reverse();
    out.push_str(&parts.join("\\\\"));
}

fn dump_value(value: &crate::registry_value::RegistryValue, out: &mut String) {
    if value.name.is_empty() {
        out.push_str("@=");
    } else {
        out.push('"');
        out.push_str(&escape_string(&value.name));
        out.push_str("\"=");
    }

    match &value.data {
        RegistryValueData::String(v) => {
            out.push('"');
            out.push_str(&escape_string(v));
            out.push('"');
        }
        RegistryValueData::ExpandString(v) => {
            out.push_str("str(2):\"");
            out.push_str(&escape_string(v));
            out.push('"');
        }
        RegistryValueData::MultiString(values) => {
            out.push_str("str(7):\"");
            let mut combined = String::new();
            for (idx, part) in values.iter().enumerate() {
                if idx > 0 {
                    combined.push_str("\\0");
                }
                combined.push_str(&escape_string(part));
            }
            combined.push_str("\\0");
            out.push_str(&combined);
            out.push('"');
        }
        RegistryValueData::Dword(v) => {
            out.push_str(&format!("dword:{:08x}", v));
        }
        RegistryValueData::Qword(v) => {
            let bytes = v.to_le_bytes();
            out.push_str("hex(b):");
            write_hex_bytes(&bytes, out, 5);
            return;
        }
        RegistryValueData::Binary(bytes, ty) => {
            if *ty == REG_BINARY {
                out.push_str("hex:");
            } else {
                out.push_str(&format!("hex({:x}):", ty));
            }
            write_hex_bytes(bytes, out, if *ty == REG_BINARY { 4 } else { 6 });
            return;
        }
    }
    out.push('\n');
}

fn write_hex_bytes(bytes: &[u8], out: &mut String, mut line_count: usize) {
    for (idx, b) in bytes.iter().enumerate() {
        out.push_str(&format!("{:02x}", b));
        line_count += 2;
        if idx + 1 != bytes.len() {
            out.push(',');
            line_count += 1;
        }
        if line_count >= 76 && idx + 1 != bytes.len() {
            out.push_str("\\\n  ");
            line_count = 2;
        }
    }
    out.push('\n');
}

fn escape_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0}' => out.push_str("\\0"),
            _ => out.push(ch),
        }
    }
    out
}

use std::rc::Rc;

