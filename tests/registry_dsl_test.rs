use winereg::*;

#[test]
fn dsl_builds_registry() {
    let reg = registry(|r| {
        r.relative_base = "\\\\REGISTRY\\\\MACHINE".into();
        r.architecture = Architecture::Win64;
        r.key("MACHINE\\SOFTWARE\\TestApp", |k| {
            k.value("Version", "1.0.0");
            k.dword("Enabled", 1);
            k.expand_string("Path", "%ProgramFiles%\\Test");
            k.multi_string("Items", vec!["a".into(), "b".into()]);
        });
    });

    assert_eq!(reg.relative_base, "\\\\REGISTRY\\\\MACHINE");
    assert_eq!(reg.architecture, Architecture::Win64);
    let key = reg.get("MACHINE\\SOFTWARE\\TestApp").unwrap();
    assert_eq!(key.borrow().get_value("Version").unwrap().reg_type(), REG_SZ);
    assert_eq!(
        key.borrow().get_value("Enabled").unwrap().raw_bytes().len(),
        4
    );
}

