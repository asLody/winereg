use crate::registry_comparator::{DiffResult, KeyPropertyChange, RegistryChange};
use crate::registry_key::{KeyNode, RegistryKey};
use crate::registry_value::RegistryValue;

#[derive(Debug, Clone)]
pub struct PatchOptions {
    pub ignore_failures: bool,
    pub create_missing_keys: bool,
    pub overwrite_existing_values: bool,
    pub delete_empty_keys: bool,
    pub validate_before_apply: bool,
}

impl Default for PatchOptions {
    fn default() -> Self {
        Self {
            ignore_failures: false,
            create_missing_keys: true,
            overwrite_existing_values: true,
            delete_empty_keys: true,
            validate_before_apply: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PatchFailure {
    pub change: RegistryChange,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct PatchResult {
    pub applied: Vec<RegistryChange>,
    pub failed: Vec<PatchFailure>,
    pub ignore_failures: bool,
}

impl PatchResult {
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }
    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }
    pub fn total_count(&self) -> usize {
        self.applied_count() + self.failed_count()
    }
    pub fn is_success(&self) -> bool {
        self.failed.is_empty() || self.ignore_failures
    }
}

pub struct RegistryPatcher;

impl RegistryPatcher {
    pub fn apply_patch(&self, target: &KeyNode, diff: &DiffResult, options: PatchOptions) -> PatchResult {
        let ordered = order_changes(&diff.changes);
        let mut applied = Vec::new();
        let mut failed = Vec::new();

        for change in ordered {
            let res = apply_change(target, &change, &options);
            match res {
                Ok(true) => applied.push(change),
                Ok(false) => {
                    failed.push(PatchFailure { change: change.clone(), reason: "Unable to apply change".into() });
                    if !options.ignore_failures {
                        break;
                    }
                }
                Err(msg) => {
                    failed.push(PatchFailure { change: change.clone(), reason: msg });
                    if !options.ignore_failures {
                        break;
                    }
                }
            }
        }

        PatchResult {
            applied,
            failed,
            ignore_failures: options.ignore_failures,
        }
    }
}

fn order_changes(changes: &[RegistryChange]) -> Vec<RegistryChange> {
    let mut additions: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::KeyAdded(_))).cloned().collect();
    additions.sort_by_key(|c| match c { RegistryChange::KeyAdded(p) => p.matches('\\').count(), _ => 0 });
    let key_mods: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::KeyModified(_, _))).cloned().collect();
    let val_adds: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::ValueAdded(_, _, _))).cloned().collect();
    let val_mods: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::ValueModified(_, _, _, _))).cloned().collect();
    let val_dels: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::ValueDeleted(_, _, _))).cloned().collect();
    let mut key_dels: Vec<_> = changes.iter().filter(|c| matches!(c, RegistryChange::KeyDeleted(_))).cloned().collect();
    key_dels.sort_by(|a, b| depth(b).cmp(&depth(a)));

    let mut ordered = Vec::new();
    ordered.extend(additions);
    ordered.extend(key_mods);
    ordered.extend(val_adds);
    ordered.extend(val_mods);
    ordered.extend(val_dels);
    ordered.extend(key_dels);
    ordered
}

fn depth(change: &RegistryChange) -> usize {
    match change {
        RegistryChange::KeyAdded(p)
        | RegistryChange::KeyDeleted(p)
        | RegistryChange::KeyModified(p, _)
        | RegistryChange::ValueAdded(p, _, _)
        | RegistryChange::ValueDeleted(p, _, _)
        | RegistryChange::ValueModified(p, _, _, _) => p.matches('\\').count(),
    }
}

fn apply_change(target: &KeyNode, change: &RegistryChange, options: &PatchOptions) -> Result<bool, String> {
    match change {
        RegistryChange::KeyAdded(path) => apply_key_added(target, path, options),
        RegistryChange::KeyDeleted(path) => apply_key_deleted(target, path),
        RegistryChange::KeyModified(path, props) => apply_key_modified(target, path, props),
        RegistryChange::ValueAdded(key_path, value_name, value) => apply_value_added(target, key_path, value_name, value.clone(), options),
        RegistryChange::ValueDeleted(key_path, value_name, _value) => apply_value_deleted(target, key_path, value_name, options),
        RegistryChange::ValueModified(key_path, value_name, old_value, new_value) => apply_value_modified(target, key_path, value_name, old_value, new_value, options),
    }
}

fn apply_key_added(target: &KeyNode, path: &str, options: &PatchOptions) -> Result<bool, String> {
    if options.create_missing_keys {
        RegistryKey::create_key_recursive(target, path);
        Ok(true)
    } else {
        let parent_path = path.rsplit_once('\\').map(|(p, _)| p.to_string()).unwrap_or_else(|| "".into());
        if parent_path.is_empty() {
            RegistryKey::create_key_recursive(target, path);
            Ok(true)
        } else if RegistryKey::find_key(target, &parent_path).is_some() {
            RegistryKey::create_key_recursive(target, path);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn apply_key_deleted(target: &KeyNode, path: &str) -> Result<bool, String> {
    let (parent_path, key_name) = path.rsplit_once('\\').map(|(p, n)| (p.to_string(), n.to_string())).unwrap_or_else(|| ("".into(), path.to_string()));
    if let Some(parent) = if parent_path.is_empty() { Some(target.clone()) } else { RegistryKey::find_key(target, &parent_path) } {
        Ok(RegistryKey::delete_subkey(&parent, &key_name, true))
    } else {
        Ok(false)
    }
}

fn apply_key_modified(target: &KeyNode, path: &str, props: &[KeyPropertyChange]) -> Result<bool, String> {
    let node = RegistryKey::find_key(target, path).ok_or_else(|| "missing key".to_string())?;
    {
        let mut guard = node.borrow_mut();
        for p in props {
            match p {
                KeyPropertyChange::ClassNameChange(_, new) => guard.class_name = new.clone(),
                KeyPropertyChange::SymlinkChange(_, new) => guard.is_symlink = *new,
                KeyPropertyChange::VolatileChange(_, new) => guard.is_volatile = *new,
            }
        }
    }
    Ok(true)
}

fn apply_value_added(target: &KeyNode, key_path: &str, value_name: &str, value: RegistryValue, options: &PatchOptions) -> Result<bool, String> {
    let key = if key_path.is_empty() {
        target.clone()
    } else if options.create_missing_keys {
        RegistryKey::create_key_recursive(target, key_path)
    } else {
        RegistryKey::find_key(target, key_path).ok_or_else(|| "missing key".to_string())?
    };

    let mut guard = key.borrow_mut();
    if !options.overwrite_existing_values && guard.get_value(value_name).is_some() {
        return Ok(false);
    }
    guard.set_value(value_name.to_string(), value);
    Ok(true)
}

fn apply_value_deleted(target: &KeyNode, key_path: &str, value_name: &str, options: &PatchOptions) -> Result<bool, String> {
    let key = RegistryKey::find_key(target, key_path).ok_or_else(|| "missing key".to_string())?;
    let removed = key.borrow_mut().delete_value(value_name);
    if removed && options.delete_empty_keys {
        delete_empty_chain(target, key_path);
    }
    Ok(removed)
}

fn apply_value_modified(target: &KeyNode, key_path: &str, value_name: &str, old_value: &RegistryValue, new_value: &RegistryValue, options: &PatchOptions) -> Result<bool, String> {
    let key = RegistryKey::find_key(target, key_path).ok_or_else(|| "missing key".to_string())?;
    let mut guard = key.borrow_mut();
    if options.validate_before_apply {
        if let Some(existing) = guard.get_value(value_name) {
            if existing.reg_type() != old_value.reg_type() || existing.raw_bytes() != old_value.raw_bytes() {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    guard.set_value(value_name.to_string(), new_value.clone());
    Ok(true)
}

fn delete_empty_chain(root: &KeyNode, path: &str) {
    if path.is_empty() {
        return;
    }
    let mut current_path = path.to_string();
    while !current_path.is_empty() {
        if let Some(node) = RegistryKey::find_key(root, &current_path) {
            let is_empty = { node.borrow().values().is_empty() && node.borrow().subkeys().is_empty() };
            if is_empty {
                let (parent_path, name) = current_path.rsplit_once('\\').map(|(p, n)| (p.to_string(), n.to_string())).unwrap_or_else(|| ("".into(), current_path.clone()));
                if let Some(parent) = if parent_path.is_empty() { Some(root.clone()) } else { RegistryKey::find_key(root, &parent_path) } {
                    if !RegistryKey::delete_subkey(&parent, &name, false) {
                        break;
                    }
                }
                current_path = parent_path;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

