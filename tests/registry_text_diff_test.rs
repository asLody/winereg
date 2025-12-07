use winereg::*;

#[test]
fn text_diff_export_and_parse_round_trip() {
    let key1 = RegistryKey::create_root();
    let key2 = RegistryKey::create_root();
    {
        let k = RegistryKey::create_key_recursive(&key2, "SOFTWARE\\Test");
        k.borrow_mut().set_value(
            "Version",
            RegistryValue::new("Version", RegistryValueData::String("1.0".into())),
        );
    }
    let comparator = RegistryComparator;
    let diff = comparator.compare_registries(&key1, &key2);
    let exporter = TextDiffExporter;
    let text = exporter.export(&diff, None, None);
    let parser = TextDiffParser;
    let parsed = parser.parse(&text).expect("parse text diff");
    assert_eq!(diff.changes.len(), parsed.changes.len());
}

#[test]
fn text_diff_export_parse_and_apply_produces_identical_registry() {
    let base = RegistryKey::create_root();
    let desired = RegistryKey::create_root();
    let desired_key = RegistryKey::create_key_recursive(&desired, "Software\\Example");
    {
        let mut guard = desired_key.borrow_mut();
        guard.set_value(
            "Version",
            RegistryValue::new("Version", RegistryValueData::String("1.2.3".into())),
        );
        guard.set_value(
            "Enabled",
            RegistryValue::new("Enabled", RegistryValueData::Dword(1)),
        );
    }

    let comparator = RegistryComparator;
    let diff = comparator.compare_registries(&base, &desired);
    let exporter = TextDiffExporter;
    let text = exporter.export(&diff, Some("old.reg"), Some("new.reg"));
    let parser = TextDiffParser;
    let parsed = parser.parse(&text).expect("parse diff text");

    let patcher = RegistryPatcher;
    let result = patcher.apply_patch(&base, &parsed, PatchOptions::default());
    assert!(result.is_success());

    let final_diff = comparator.compare_registries(&base, &desired);
    assert!(!final_diff.has_changes());
}

