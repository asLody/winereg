use winereg::*;

#[test]
fn should_create_basic_registry_key() {
    let key = RegistryKey::create_root();
    assert!(key.borrow().subkeys().is_empty());
    assert!(key.borrow().values().is_empty());
}

#[test]
fn should_create_and_set_string_value() {
    let root = RegistryKey::create_root();
    let mut guard = root.borrow_mut();
    guard.set_value(
        "TestValue",
        RegistryValue::new("TestValue", RegistryValueData::String("Hello World".into())),
    );
    let retrieved = guard.get_value("TestValue").unwrap();
    assert_eq!(
        "Hello World",
        match retrieved.data {
            RegistryValueData::String(ref s) => s,
            _ => panic!("expected string"),
        }
    );
}

#[test]
fn should_create_subkeys() {
    let root = RegistryKey::create_root();
    let child = RegistryKey::create_subkey(&root, "Child");
    assert_eq!("Child", child.borrow().name);
    assert!(RegistryKey::find_key(&root, "Child").is_some());
}

#[test]
fn should_create_comparator_and_patcher() {
    let comparator = RegistryComparator;
    let patcher = RegistryPatcher;
    // basic sanity
    let diff = comparator.compare_registries(&RegistryKey::create_root(), &RegistryKey::create_root());
    let res = patcher.apply_patch(&RegistryKey::create_root(), &diff, PatchOptions::default());
    assert!(res.is_success());
}

