use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

use crate::registry_value::RegistryValue;
use crate::{
    registry_comparator::{DiffResult, RegistryComparator},
    registry_patcher::{PatchOptions, PatchResult, RegistryPatcher},
    registry_text_diff::{TextDiffExporter, TextDiffParser},
};

pub type KeyNode = Rc<RefCell<RegistryKey>>;

#[derive(Debug)]
pub struct RegistryKey {
    pub name: String,
    pub class_name: Option<String>,
    pub modification_time: u64,
    pub is_symlink: bool,
    pub is_volatile: bool,
    pub is_dirty: bool,
    parent: Option<Weak<RefCell<RegistryKey>>>,
    subkeys: BTreeMap<String, KeyNode>,
    values: BTreeMap<String, RegistryValue>,
}

impl RegistryKey {
    pub fn create_root() -> KeyNode {
        let root = Rc::new(RefCell::new(Self {
            name: String::new(),
            class_name: None,
            modification_time: 0,
            is_symlink: false,
            is_volatile: false,
            is_dirty: false,
            parent: None,
            subkeys: BTreeMap::new(),
            values: BTreeMap::new(),
        }));
        root
    }

    fn new_with_parent(parent: &KeyNode, name: impl Into<String>) -> KeyNode {
        let node = Rc::new(RefCell::new(Self {
            name: name.into(),
            class_name: None,
            modification_time: 0,
            is_symlink: false,
            is_volatile: false,
            is_dirty: true,
            parent: Some(Rc::downgrade(parent)),
            subkeys: BTreeMap::new(),
            values: BTreeMap::new(),
        }));
        node
    }

    pub fn subkeys(&self) -> &BTreeMap<String, KeyNode> {
        &self.subkeys
    }

    pub fn values(&self) -> &BTreeMap<String, RegistryValue> {
        &self.values
    }

    pub fn get_subkey(&self, name: &str) -> Option<KeyNode> {
        self.subkeys.get(&normalize(name)).cloned()
    }

    pub fn parent(&self) -> Option<KeyNode> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    pub fn get_value(&self, name: &str) -> Option<&RegistryValue> {
        self.values.get(&normalize(name))
    }

    pub fn set_value(&mut self, name: impl Into<String>, value: RegistryValue) {
        let key = normalize(&name.into());
        self.values.insert(key, value);
        self.mark_dirty();
    }

    pub fn set_value_for_loading(&mut self, name: impl Into<String>, value: RegistryValue) {
        let key = normalize(&name.into());
        self.values.insert(key, value);
    }

    pub fn delete_value(&mut self, name: &str) -> bool {
        let key = normalize(name);
        let removed = self.values.remove(&key).is_some();
        if removed {
            self.mark_dirty();
        }
        removed
    }

    /// Result-returning variant for deletion, returning an error when value is absent.
    pub fn try_delete_value(&mut self, name: &str) -> Result<(), String> {
        if self.delete_value(name) {
            Ok(())
        } else {
            Err(format!("value '{}' not found", name))
        }
    }

    pub fn create_subkey(parent: &KeyNode, name: impl Into<String>) -> KeyNode {
        let name_str = name.into();
        let key = normalize(&name_str);
        if let Some(existing) = parent.borrow().subkeys.get(&key) {
            return existing.clone();
        }
        let new = Self::new_with_parent(parent, name_str);
        parent.borrow_mut().subkeys.insert(key, new.clone());
        parent.borrow_mut().mark_dirty();
        new
    }

    pub fn create_key_recursive(parent: &KeyNode, path: &str) -> KeyNode {
        if path.is_empty() {
            return parent.clone();
        }
        let mut current = parent.clone();
        for segment in path.split('\\').filter(|s| !s.is_empty()) {
            let next = {
                let mut guard = current.borrow_mut();
                if let Some(existing) = guard.subkeys.get(&normalize(segment)) {
                    existing.clone()
                } else {
                    let new = Self::new_with_parent(&current, segment);
                    guard.subkeys.insert(normalize(segment), new.clone());
                    guard.mark_dirty();
                    new
                }
            };
            current = next;
        }
        current
    }

    pub fn find_key(parent: &KeyNode, path: &str) -> Option<KeyNode> {
        if path.is_empty() {
            return Some(parent.clone());
        }
        let mut current = parent.clone();
        for segment in path.split('\\').filter(|s| !s.is_empty()) {
            let next = {
                let guard = current.borrow();
                guard.get_subkey(segment)
            };
            match next {
                Some(n) => current = n,
                None => return None,
            }
        }
        Some(current)
    }

    /// Return a snapshot of subkeys as (name, KeyNode) pairs to avoid RefCell borrow issues.
    pub fn snapshot_subkeys(node: &KeyNode) -> Vec<(String, KeyNode)> {
        node.borrow()
            .subkeys()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Return a snapshot of values as (name, RegistryValue) pairs to avoid RefCell borrow issues.
    pub fn snapshot_values(node: &KeyNode) -> Vec<(String, RegistryValue)> {
        node.borrow()
            .values()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Attempt to delete a subkey; returns Ok if deleted or Err if missing/non-empty (when recursive is false).
    pub fn try_delete_subkey(parent: &KeyNode, name: &str, recursive: bool) -> Result<(), String> {
        if Self::delete_subkey(parent, name, recursive) {
            Ok(())
        } else {
            Err(format!("subkey '{}' not deleted", name))
        }
    }

    pub fn delete_subkey(parent: &KeyNode, name: &str, recursive: bool) -> bool {
        let key = normalize(name);
        let mut guard = parent.borrow_mut();
        if !recursive {
            if let Some(sub) = guard.subkeys.get(&key) {
                if !sub.borrow().subkeys.is_empty() {
                    return false;
                }
            } else {
                return false;
            }
        }
        let removed = guard.subkeys.remove(&key).is_some();
        if removed {
            guard.mark_dirty();
        }
        removed
    }

    pub fn get_full_path(node: &KeyNode) -> String {
        let mut parts = Vec::new();
        let mut current = Some(node.clone());
        while let Some(n) = current {
            let guard = n.borrow();
            if !guard.name.is_empty() {
                parts.push(guard.name.clone());
            }
            current = guard.parent.as_ref().and_then(|p| p.upgrade());
        }
        parts.reverse();
        parts.join("\\\\")
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
        let mut current = self.parent.clone();
        while let Some(parent) = current.and_then(|p| p.upgrade()) {
            parent.borrow_mut().is_dirty = true;
            current = parent.borrow().parent.clone();
        }
    }
}

fn normalize(name: &str) -> String {
    name.to_ascii_uppercase()
}

/// Convenience extensions to mirror KRegEdit's extension functions.
pub trait RegistryKeyExt {
    fn apply_patch_with(&self, diff: &DiffResult, options: PatchOptions) -> PatchResult;
    fn apply_patch(&self, diff: &DiffResult) -> PatchResult;
    fn apply_text_patch(&self, text: &str, options: PatchOptions) -> Result<PatchResult, String>;
    fn compare_with(&self, other: &KeyNode) -> DiffResult;
    fn export_diff_text(
        &self,
        other: &KeyNode,
        from_file: Option<&str>,
        to_file: Option<&str>,
    ) -> String;
}

impl RegistryKeyExt for KeyNode {
    fn apply_patch_with(&self, diff: &DiffResult, options: PatchOptions) -> PatchResult {
        let patcher = RegistryPatcher;
        patcher.apply_patch(self, diff, options)
    }

    fn apply_patch(&self, diff: &DiffResult) -> PatchResult {
        self.apply_patch_with(diff, PatchOptions::default())
    }

    fn apply_text_patch(&self, text: &str, options: PatchOptions) -> Result<PatchResult, String> {
        let parser = TextDiffParser;
        let diff = parser.parse(text)?;
        let patcher = RegistryPatcher;
        Ok(patcher.apply_patch(self, &diff, options))
    }

    fn compare_with(&self, other: &KeyNode) -> DiffResult {
        let comparator = RegistryComparator;
        comparator.compare_registries(self, other)
    }

    fn export_diff_text(
        &self,
        other: &KeyNode,
        from_file: Option<&str>,
        to_file: Option<&str>,
    ) -> String {
        let comparator = RegistryComparator;
        let exporter = TextDiffExporter;
        let diff = comparator.compare_registries(self, other);
        exporter.export(&diff, from_file, to_file)
    }
}

