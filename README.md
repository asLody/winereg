## winereg

Rust library for parsing, writing, diffing, patching, and scripting Wine registry files.

### Crate Overview
- Parse Wine `.reg` text/files into an in-memory tree.
- Serialize back to `.reg` (atomic file write support).
- Compare registries and export/parse text diffs.
- Apply diffs/patches with configurable options.
- Lightweight DSL for building or modifying registries.

### Install
Add to `Cargo.toml`:
```toml
[dependencies]
winereg = "0.1.0"
```

Use in code:
```rust
use winereg::*;
```

### Core Types
- `KeyNode` / `RegistryKey`
  - `RegistryKey::create_root()`
  - `RegistryKey::create_subkey(&parent, name)` / `create_key_recursive(&parent, path)`
  - `RegistryKey::find_key(&root, path) -> Option<KeyNode>`
  - `set_value(name, RegistryValue)`; `try_delete_value(name) -> Result<()>`
  - `delete_subkey(parent, name, recursive) -> bool`; `try_delete_subkey(parent, name, recursive) -> Result<()>`
  - Snapshots to avoid borrow issues: `snapshot_subkeys(&KeyNode)`, `snapshot_values(&KeyNode)`
  - `get_full_path(&KeyNode)` returns the joined registry path.
- Values
  - `RegistryValue::new(name, RegistryValueData::*)`
  - Variants: `String`, `ExpandString`, `MultiString(Vec<String>)`, `Dword(u32)`, `Qword(u64)`, `Binary(Vec<u8>, u32)`
  - Common type constants: `REG_SZ`, `REG_EXPAND_SZ`, `REG_MULTI_SZ`, `REG_DWORD`, `REG_QWORD`, `REG_BINARY`

### Parsing & Writing (RegistryEditor)
- `RegistryEditor::load_from_file(path) -> Result<LoadResult, ParseError>`
- `RegistryEditor::load_from_text(text) -> Result<LoadResult, ParseError>`
- `RegistryEditor::write_to_file_with_options(key, path, EditorOptions) -> io::Result<()>`
- `RegistryEditor::write_to_file_default(key, path) -> io::Result<()>`
- `RegistryEditor::write_to_string_with_options(key, EditorOptions) -> String`
- `RegistryEditor::write_to_string_default(key) -> String`
- `EditorOptions { relative_base: String, architecture: Architecture }` (`Default`: empty base + `Unknown`)
- `Architecture`: `Unknown`, `Win32`, `Win64`

### Diff & Patch
- Compare: `RegistryComparator.compare_registries(left, right) -> DiffResult`
- Text diff export/parse:
  - `TextDiffExporter.export(&diff, from: Option<&str>, to: Option<&str>) -> String`
  - `TextDiffParser.parse(text) -> Result<DiffResult, String>`
- Apply patch:
  - `RegistryPatcher.apply_patch(target, &diff, PatchOptions) -> PatchResult`
  - `PatchOptions { ignore_failures, create_missing_keys, overwrite_existing_values, delete_empty_keys, validate_before_apply }` (`Default`: ignore_failures=false, create_missing_keys=true, overwrite_existing_values=true, delete_empty_keys=true, validate_before_apply=false)
  - `PatchResult { applied, failed, ignore_failures }`
    - `is_success()` respects `ignore_failures`
    - `applied_count()`, `failed_count()`, `total_count()`
- Convenience on `KeyNode` via `RegistryKeyExt`:
  - `apply_patch(&self, &DiffResult)`
  - `apply_patch_with(&self, &DiffResult, PatchOptions)`
  - `apply_text_patch(&self, text, options) -> Result<PatchResult, String>`
  - `compare_with(&self, other) -> DiffResult`
  - `export_diff_text(&self, other, from, to) -> String`

### DSL (optional)
- `registry(|ctx| { ... }) -> RegistryResult`
  - Set `ctx.relative_base`, `ctx.architecture`
  - `ctx.key("PATH", |k| { ... })`, `ctx.root(|k| { ... })`
- `RegistryKeyDsl` helpers: `value`, `dword`, `qword`, `binary`, `expand_string`, `multi_string`, `delete_value`, `delete_key(recursive)`, `replace_key`, `update_time`
- Mutating existing registry: `modify_registry(registry_result, |k| { ... })`

### Quick Examples
Load, tweak, save:
```rust
use winereg::*;

let loaded = RegistryEditor::load_from_file("user.reg")?;
let root = loaded.root_key.clone();
let key = RegistryKey::create_key_recursive(&root, "Software\\MyApp");
key.borrow_mut().set_value(
    "Version",
    RegistryValue::new("Version", RegistryValueData::String("2.0".into()))
);
RegistryEditor::write_to_file_default(&root, "user_out.reg")?;
```

Compare and patch:
```rust
let base = RegistryEditor::load_from_file("user.reg")?.root_key;
let desired = RegistryEditor::load_from_file("user_new.reg")?.root_key;
let diff = RegistryComparator.compare_registries(&base, &desired);
let result = RegistryPatcher.apply_patch(&base, &diff, PatchOptions::default());
assert!(result.is_success());
```

Text diff round-trip:
```rust
let diff = RegistryComparator.compare_registries(&RegistryKey::create_root(), &desired);
let text = TextDiffExporter.export(&diff, Some("old.reg"), Some("new.reg"));
let parsed = TextDiffParser.parse(&text)?;
let apply_res = RegistryPatcher.apply_patch(&RegistryKey::create_root(), &parsed, PatchOptions::default());
assert!(apply_res.is_success());
```

### Text Diff Format (Readable Patch)
The text diff is a compact, section-based format.

#### Anatomy
- Header lines:
  - `# Registry Patch File`
  - Optional: `# Generated: <timestamp>`
  - Optional: `# FROM: <file1>` / `# TO: <file2>`
- Sections:
  - `[ROOT]` for the virtual root (empty path)
  - `[HKEY_LOCAL_MACHINE]`, `[Software\Classes]`, etc.
- Inside each section, change lines:
  - Key add: `+key:<Name>`
  - Key delete: `-key:<Name>`
  - Key properties:
    - `~className:<old>-><new>` (values quoted when needed)
    - `~isSymlink:false->true`
    - `~isVolatile:false->true`
  - Value add/delete:
    - `+"Name"=<typed payload>`
    - `-"Name"=<typed payload>`
  - Value modify:
    - `~"Name"=<old typed payload>-><new typed payload>`
- Value payload encodings:
  - String: `string:"text"` (escapes: `\"`, `\\`, `\n`, `\r`, `\t`, `\0`)
  - Expand string: `expand_string:"text"`
  - Multi-string: `multi_string:["a","b","c"]`
  - Dword: `dword:00112233` (hex, 8 digits)
  - Qword: `qword:0011223344556677` (hex, 16 digits)
  - Binary:
    - `hex:01,02,ff` (REG_BINARY)
    - `hex(ffff1003):01,02` (explicit type in hex)

#### Example Patch
```text
# Registry Patch File
# FROM: old.reg
# TO: new.reg

[ROOT]
+key:HKEY_LOCAL_MACHINE

[HKEY_LOCAL_MACHINE]
~className:"OldClass"->"NewClass"

[HKEY_LOCAL_MACHINE\Software]
+"StringValue"=string:"hello\nworld"
+"DwordValue"=dword:0000002a
~"Mode"=string:"old"->string:"new"
-"Obsolete"=dword:00000001
```

#### Parsing & Applying
- Parse: `let diff = TextDiffParser.parse(text)?;`
- Apply: `RegistryPatcher.apply_patch(target, &diff, options);`
  - `PatchOptions` control behavior (`ignore_failures`, `create_missing_keys`, `overwrite_existing_values`, `delete_empty_keys`, `validate_before_apply`).

