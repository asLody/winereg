use crate::registry_key::KeyNode;
use crate::registry_value::RegistryValue;

#[derive(Debug, Clone)]
pub enum RegistryChange {
    KeyAdded(String),
    KeyDeleted(String),
    KeyModified(String, Vec<KeyPropertyChange>),
    ValueAdded(String, String, RegistryValue),
    ValueDeleted(String, String, RegistryValue),
    ValueModified(String, String, RegistryValue, RegistryValue),
}

#[derive(Debug, Clone)]
pub enum KeyPropertyChange {
    ClassNameChange(Option<String>, Option<String>),
    SymlinkChange(bool, bool),
    VolatileChange(bool, bool),
}

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub changes: Vec<RegistryChange>,
}

impl DiffResult {
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn added_keys(&self) -> Vec<&RegistryChange> {
        self.changes.iter().filter(|c| matches!(c, RegistryChange::KeyAdded(_))).collect()
    }
}

pub struct RegistryComparator;

impl RegistryComparator {
    pub fn compare_registries(&self, left: &KeyNode, right: &KeyNode) -> DiffResult {
        let mut changes = Vec::new();
        compare_keys(Some(left.clone()), Some(right.clone()), String::new(), &mut changes);
        DiffResult { changes }
    }
}

fn compare_keys(left: Option<KeyNode>, right: Option<KeyNode>, path: String, changes: &mut Vec<RegistryChange>) {
    match (left, right) {
        (None, Some(r)) => {
            changes.push(RegistryChange::KeyAdded(path.clone()));
            add_subtree_added(&r, &path, changes);
        }
        (Some(l), None) => {
            changes.push(RegistryChange::KeyDeleted(path.clone()));
            add_subtree_deleted(&l, &path, changes);
        }
        (Some(l), Some(r)) => {
            let l_guard = l.borrow();
            let r_guard = r.borrow();

            let mut prop_changes = Vec::new();
            if l_guard.class_name != r_guard.class_name {
                prop_changes.push(KeyPropertyChange::ClassNameChange(l_guard.class_name.clone(), r_guard.class_name.clone()));
            }
            if l_guard.is_symlink != r_guard.is_symlink {
                prop_changes.push(KeyPropertyChange::SymlinkChange(l_guard.is_symlink, r_guard.is_symlink));
            }
            if l_guard.is_volatile != r_guard.is_volatile {
                prop_changes.push(KeyPropertyChange::VolatileChange(l_guard.is_volatile, r_guard.is_volatile));
            }
            drop(l_guard);
            drop(r_guard);
            if !prop_changes.is_empty() {
                changes.push(RegistryChange::KeyModified(path.clone(), prop_changes));
            }

            compare_values(&l, &r, &path, changes);
            compare_subkeys(&l, &r, &path, changes);
        }
        (None, None) => {}
    }
}

fn compare_values(left: &KeyNode, right: &KeyNode, path: &str, changes: &mut Vec<RegistryChange>) {
    let l_vals = left.borrow().values().clone();
    let r_vals = right.borrow().values().clone();

    for (name, rv) in r_vals.iter() {
        if !l_vals.contains_key(name) {
            changes.push(RegistryChange::ValueAdded(path.to_string(), rv.name.clone(), rv.clone()));
        }
    }
    for (name, lv) in l_vals.iter() {
        if !r_vals.contains_key(name) {
            changes.push(RegistryChange::ValueDeleted(path.to_string(), lv.name.clone(), lv.clone()));
        }
    }
    for (name, lv) in l_vals.iter() {
        if let Some(rv) = r_vals.get(name) {
            if !values_equal(lv, rv) {
                changes.push(RegistryChange::ValueModified(path.to_string(), lv.name.clone(), lv.clone(), rv.clone()));
            }
        }
    }
}

fn compare_subkeys(left: &KeyNode, right: &KeyNode, path: &str, changes: &mut Vec<RegistryChange>) {
    let l_sub = left.borrow().subkeys().clone();
    let r_sub = right.borrow().subkeys().clone();
    let mut names = l_sub.keys().cloned().collect::<Vec<_>>();
    for name in r_sub.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names.sort();
    for name in names {
        let sub_path = if path.is_empty() { name.clone() } else { format!("{}\\{}", path, name) };
        compare_keys(l_sub.get(&name).cloned(), r_sub.get(&name).cloned(), sub_path, changes);
    }
}

fn add_subtree_added(node: &KeyNode, path: &str, changes: &mut Vec<RegistryChange>) {
    let guard = node.borrow();
    for v in guard.values().values() {
        changes.push(RegistryChange::ValueAdded(path.to_string(), v.name.clone(), v.clone()));
    }
    for (name, sub) in guard.subkeys() {
        let sub_path = if path.is_empty() { name.clone() } else { format!("{}\\{}", path, name) };
        changes.push(RegistryChange::KeyAdded(sub_path.clone()));
        add_subtree_added(sub, &sub_path, changes);
    }
}

fn add_subtree_deleted(node: &KeyNode, path: &str, changes: &mut Vec<RegistryChange>) {
    let guard = node.borrow();
    for v in guard.values().values() {
        changes.push(RegistryChange::ValueDeleted(path.to_string(), v.name.clone(), v.clone()));
    }
    for (name, sub) in guard.subkeys() {
        let sub_path = if path.is_empty() { name.clone() } else { format!("{}\\{}", path, name) };
        changes.push(RegistryChange::KeyDeleted(sub_path.clone()));
        add_subtree_deleted(sub, &sub_path, changes);
    }
}

fn values_equal(a: &RegistryValue, b: &RegistryValue) -> bool {
    a.reg_type() == b.reg_type() && a.raw_bytes() == b.raw_bytes()
}

