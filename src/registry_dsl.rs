use crate::architecture::Architecture;
use crate::registry_key::{KeyNode, RegistryKey};
use crate::registry_parser::RegistryParser;
use crate::registry_utils::set_current_time_recursive;
use crate::registry_value::{RegistryValue, RegistryValueData};
use crate::registry_writer::RegistryWriter;

pub struct RegistryResult {
    pub root_key: KeyNode,
    pub relative_base: String,
    pub architecture: Architecture,
}

impl RegistryResult {
    pub fn write_to_file(&self, path: &str) {
        let writer = RegistryWriter {
            relative_base: self.relative_base.clone(),
            architecture: self.architecture,
        };
        let _ = writer.write_to_file(&self.root_key, path);
    }

    pub fn write_to_string(&self) -> String {
        let writer = RegistryWriter {
            relative_base: self.relative_base.clone(),
            architecture: self.architecture,
        };
        writer.write_to_string(&self.root_key)
    }

    pub fn update_times(&self) -> &Self {
        set_current_time_recursive(&self.root_key);
        self
    }

    pub fn modify<F>(&self, f: F) -> &Self
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let mut dsl = RegistryKeyDsl {
            key: self.root_key.clone(),
        };
        f(&mut dsl);
        self
    }

    pub fn get(&self, path: &str) -> Option<KeyNode> {
        RegistryKey::find_key(&self.root_key, path)
    }

    pub fn invoke<F>(&self, path: &str, f: F) -> KeyNode
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let node = RegistryKey::create_key_recursive(&self.root_key, path);
        let mut dsl = RegistryKeyDsl { key: node.clone() };
        f(&mut dsl);
        node
    }
}

pub struct RegistryDslContext {
    root: KeyNode,
    pub relative_base: String,
    pub architecture: Architecture,
}

impl RegistryDslContext {
    pub fn new() -> Self {
        Self {
            root: RegistryKey::create_root(),
            relative_base: String::new(),
            architecture: Architecture::Unknown,
        }
    }

    pub fn root<F>(&mut self, f: F)
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let mut dsl = RegistryKeyDsl {
            key: self.root.clone(),
        };
        f(&mut dsl);
    }

    pub fn key<F>(&mut self, path: &str, f: F)
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let node = RegistryKey::create_key_recursive(&self.root, path);
        let mut dsl = RegistryKeyDsl { key: node };
        f(&mut dsl);
    }

    pub fn build(self) -> RegistryResult {
        RegistryResult {
            root_key: self.root,
            relative_base: self.relative_base,
            architecture: self.architecture,
        }
    }
}

pub struct RegistryKeyDsl {
    pub key: KeyNode,
}

impl RegistryKeyDsl {
    pub fn key<F>(&mut self, path: &str, f: F)
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let node = RegistryKey::create_key_recursive(&self.key, path);
        let mut dsl = RegistryKeyDsl { key: node };
        f(&mut dsl);
    }

    pub fn class_name(&mut self, name: Option<String>) {
        self.key.borrow_mut().class_name = name;
    }

    pub fn is_symlink(&mut self, v: bool) {
        self.key.borrow_mut().is_symlink = v;
    }

    pub fn is_volatile(&mut self, v: bool) {
        self.key.borrow_mut().is_volatile = v;
    }

    pub fn value(&mut self, name: &str, value: &str) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::String(value.to_string())),
        );
    }

    pub fn dword(&mut self, name: &str, value: i32) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::Dword(value as u32)),
        );
    }

    pub fn qword(&mut self, name: &str, value: i64) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::Qword(value as u64)),
        );
    }

    pub fn binary(&mut self, name: &str, data: &[u8]) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::Binary(data.to_vec(), crate::registry_value::REG_BINARY)),
        );
    }

    pub fn expand_string(&mut self, name: &str, value: &str) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::ExpandString(value.to_string())),
        );
    }

    pub fn multi_string(&mut self, name: &str, values: Vec<String>) {
        self.key.borrow_mut().set_value(
            name.to_string(),
            RegistryValue::new(name.to_string(), RegistryValueData::MultiString(values)),
        );
    }

    pub fn delete_value(&mut self, name: &str) -> bool {
        self.key.borrow_mut().delete_value(name)
    }

    pub fn delete_key(&mut self, name: &str, recursive: bool) -> bool {
        RegistryKey::delete_subkey(&self.key, name, recursive)
    }

    pub fn replace_key<F>(&mut self, path: &str, f: F)
    where
        F: FnOnce(&mut RegistryKeyDsl),
    {
        let node = RegistryKey::create_key_recursive(&self.key, path);
        let keys: Vec<String> = node.borrow().subkeys().keys().cloned().collect();
        for k in keys {
            RegistryKey::delete_subkey(&node, &k, true);
        }
        let vals: Vec<String> = node.borrow().values().keys().cloned().collect();
        {
            let mut guard = node.borrow_mut();
            for v in vals {
                guard.delete_value(&v);
            }
        }
        let mut dsl = RegistryKeyDsl { key: node };
        f(&mut dsl);
    }

    pub fn update_time(&mut self) {
        crate::registry_utils::set_current_time_recursive(&self.key);
    }

    pub fn get_key(&self) -> KeyNode {
        self.key.clone()
    }
}

pub fn registry<F>(f: F) -> RegistryResult
where
    F: FnOnce(&mut RegistryDslContext),
{
    let mut ctx = RegistryDslContext::new();
    f(&mut ctx);
    ctx.build()
}

pub fn load_registry(path: &str) -> RegistryResult {
    let parser = RegistryParser;
    let result = parser.load_from_file(path).expect("failed to load registry");
    RegistryResult {
        root_key: result.root_key,
        relative_base: result.relative_base,
        architecture: result.architecture,
    }
}

pub fn modify_registry<F>(registry: RegistryResult, f: F) -> RegistryResult
where
    F: FnOnce(&mut RegistryKeyDsl),
{
    let mut dsl = RegistryKeyDsl {
        key: registry.root_key.clone(),
    };
    f(&mut dsl);
    registry
}

