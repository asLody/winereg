use winereg::*;

fn resource_path(name: &str) -> String {
    format!("{}/tests/resources/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn patcher_applies_changes() {
    let target = RegistryKey::create_root();
    let mut changes = Vec::new();
    changes.push(RegistryChange::KeyAdded("SOFTWARE\\NewApp".into()));
    changes.push(RegistryChange::ValueAdded(
        "SOFTWARE\\NewApp".into(),
        "Version".into(),
        RegistryValue::new("Version", RegistryValueData::String("1.0".into())),
    ));
    let diff = DiffResult { changes };
    let patcher = RegistryPatcher;
    let result = patcher.apply_patch(&target, &diff, PatchOptions::default());
    assert!(result.is_success());
    let key = RegistryKey::find_key(&target, "SOFTWARE\\NewApp").unwrap();
    assert!(key.borrow().get_value("Version").is_some());
}

#[test]
fn patcher_respects_create_missing_keys_flag() {
    let target = RegistryKey::create_root();
    let diff = DiffResult {
        changes: vec![RegistryChange::KeyAdded("SOFTWARE\\Missing\\Child".into())],
    };
    let patcher = RegistryPatcher;
    let options = PatchOptions {
        create_missing_keys: false,
        ..PatchOptions::default()
    };

    let result = patcher.apply_patch(&target, &diff, options);

    assert!(!result.is_success());
    assert_eq!(0, result.applied_count());
    assert!(result.failed_count() >= 1);
    assert!(RegistryKey::find_key(&target, "SOFTWARE\\Missing\\Child").is_none());
}

#[test]
fn patcher_can_delete_empty_key_chains() {
    let root = RegistryKey::create_root();
    let leaf = RegistryKey::create_key_recursive(&root, "SOFTWARE\\Temp\\Leaf");
    leaf.borrow_mut().set_value(
        "Only",
        RegistryValue::new("Only", RegistryValueData::String("tmp".into())),
    );

    let diff = DiffResult {
        changes: vec![RegistryChange::ValueDeleted(
            "SOFTWARE\\Temp\\Leaf".into(),
            "Only".into(),
            RegistryValue::new("Only", RegistryValueData::String("tmp".into())),
        )],
    };
    let patcher = RegistryPatcher;
    let options = PatchOptions {
        delete_empty_keys: true,
        ..PatchOptions::default()
    };
    let result = patcher.apply_patch(&root, &diff, options);

    assert!(result.is_success());
    assert_eq!(1, result.applied_count());
    assert!(RegistryKey::find_key(&root, "SOFTWARE\\Temp\\Leaf").is_none());
    assert!(RegistryKey::find_key(&root, "SOFTWARE\\Temp").is_none());
}

#[test]
fn can_apply_real_vcredist_patch_file() {
    let patch_path = resource_path("vcredist.rph");
    let content = std::fs::read_to_string(&patch_path).expect("read vcredist.rph");

    let parser = TextDiffParser;
    let diff = parser.parse(&content).expect("parse vcredist patch");

    let target = RegistryKey::create_root();
    let patcher = RegistryPatcher;
    let result = patcher.apply_patch(&target, &diff, PatchOptions::default());

    assert!(result.is_success());
    assert_eq!(0, result.failed_count());
    assert!(result.applied_count() > 0);
    assert!(RegistryKey::find_key(
        &target,
        "Software\\Classes\\Installer\\Dependencies"
    )
    .is_some());
}

