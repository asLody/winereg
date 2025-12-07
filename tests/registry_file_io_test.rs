use winereg::*;

fn resource_path(name: &str) -> String {
    format!("{}/tests/resources/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn normalize(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n").trim().to_string()
}

#[test]
fn load_and_save_roundtrip_user_reg() {
    let path = resource_path("user.reg");
    let parser = RegistryParser;
    let loaded = parser.load_from_file(&path).expect("parse user.reg");
    let writer = RegistryWriter {
        relative_base: loaded.relative_base.clone(),
        architecture: loaded.architecture,
    };
    let written = writer.write_to_string(&loaded.root_key);
    let reparsed = parser.load_from_text(&written).expect("reparse user.reg");
    assert_eq!(count_keys(&loaded.root_key), count_keys(&reparsed.root_key));
    assert_eq!(
        count_values(&loaded.root_key),
        count_values(&reparsed.root_key)
    );
}

#[test]
fn load_and_save_roundtrip_system_reg() {
    let path = resource_path("system.reg");
    let parser = RegistryParser;
    let loaded = parser.load_from_file(&path).expect("parse system.reg");
    let writer = RegistryWriter {
        relative_base: loaded.relative_base.clone(),
        architecture: loaded.architecture,
    };
    let written = writer.write_to_string(&loaded.root_key);
    let reparsed = parser.load_from_text(&written).expect("reparse system.reg");
    assert_eq!(count_keys(&loaded.root_key), count_keys(&reparsed.root_key));
    assert_eq!(
        count_values(&loaded.root_key),
        count_values(&reparsed.root_key)
    );
}

#[test]
fn load_and_save_roundtrip_userdef_reg_matches_original() {
    let path = resource_path("userdef.reg");
    let original = std::fs::read_to_string(&path).expect("read userdef.reg");
    let parser = RegistryParser;
    let loaded = parser.load_from_file(&path).expect("parse userdef.reg");
    let writer = RegistryWriter {
        relative_base: loaded.relative_base.clone(),
        architecture: loaded.architecture,
    };
    let written = writer.write_to_string(&loaded.root_key);
    assert_eq!(normalize(&original), normalize(&written));
}

#[test]
fn parser_handles_architecture_and_relative_base_from_text() {
    let reg_text = r#"WINE REGISTRY Version 2
;; All keys relative to HKEY_LOCAL_MACHINE
#arch=win64

[Software\\ArchTest]
"QWORD"=hex(b):01,00,00,00,00,00,00,00
"#;
    let parser = RegistryParser;
    let loaded = parser.load_from_text(reg_text).expect("parse text");
    assert_eq!(loaded.architecture, Architecture::Win64);
    assert_eq!(loaded.relative_base, "HKEY_LOCAL_MACHINE");
    assert!(RegistryKey::find_key(&loaded.root_key, "Software\\ArchTest").is_some());
}

#[test]
fn parser_handles_minimal_valid_text() {
    let reg_text = r#"WINE REGISTRY Version 2
;; All keys relative to HKEY_CURRENT_USER

[Software\\TextCase]
"Value"="Hello"
"#;
    let parser = RegistryParser;
    let loaded = parser.load_from_text(reg_text).expect("parse text");
    assert_eq!(loaded.relative_base, "HKEY_CURRENT_USER");
    let key = RegistryKey::find_key(&loaded.root_key, "Software\\TextCase").expect("key exists");
    let val_bytes = {
        let guard = key.borrow();
        guard
            .get_value("Value")
            .expect("value exists")
            .raw_bytes()
    };
    assert_eq!(
        val_bytes,
        RegistryValue::new("Value", RegistryValueData::String("Hello".into())).raw_bytes()
    );
}

fn count_keys(node: &KeyNode) -> usize {
    let mut total = 1;
    for child in node.borrow().subkeys().values() {
        total += count_keys(child);
    }
    total
}

fn count_values(node: &KeyNode) -> usize {
    let mut total = node.borrow().values().len();
    for child in node.borrow().subkeys().values() {
        total += count_values(child);
    }
    total
}

